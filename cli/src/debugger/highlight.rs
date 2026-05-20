//! Syntax highlighting for the source + instruction panes.
//!
//! Backed by [`syntect`] (Sublime-syntax engine, used by `bat`, `delta`,
//! `gitui`) with [`two-face`] for grammar coverage and [`syntect_tui`] for
//! the `syntect::highlighting::Style → ratatui::style::Style` conversion.
//! No tokenizer or grammar is implemented here — we don't ship any custom
//! parsing logic.
//!
//! `two-face` doesn't yet ship a SBPF grammar. We use its bundled
//! GAS-assembly syntax for the disassembly pane: it correctly colors
//! registers (`r0`-`r11`), immediates (`0x...`, decimals), labels,
//! brackets, and `;` comments. SBPF mnemonics like `mov64` / `lddw` /
//! `ja` aren't in GAS's keyword set, so they render in the default
//! foreground — acceptable, and we keep the option open to swap in a
//! dedicated SBPF syntax later without changing call sites.

use {
    ratatui::text::{Line, Span},
    std::sync::OnceLock,
    syntect::{
        easy::HighlightLines,
        highlighting::{Theme, ThemeSet},
        parsing::{syntax_definition::SyntaxDefinition, SyntaxReference, SyntaxSet},
    },
    syntect_tui::into_span,
    terminal_colorsaurus::{theme_mode, QueryOptions, ThemeMode},
};

/// Sublime-syntax grammar for Solana SBPF disassembly. Compiled into
/// syntect's engine at startup; declarative — no Rust code parses asm.
const SBPF_SYNTAX_YAML: &str = include_str!("sbpf.sublime-syntax");

/// Terminal background mode detected at startup. `bat` / `delta` both
/// detect once before any screen takeover — in-TUI detection can race
/// the OSC 11 reply against ratatui's own output. Set via
/// [`detect_theme_mode_once`] from the entrypoint, read lazily by [`ctx`].
static DETECTED_MODE: OnceLock<ThemeMode> = OnceLock::new();

/// Probe the terminal once for its background color and cache the result.
/// Call this exactly once, before the ratatui terminal guard takes over
/// stdout. Idempotent: subsequent calls are no-ops.
///
/// On detection failure (non-TTY, query unsupported, timeout) the cache
/// is set to [`ThemeMode::Dark`], which matches the most common dev-terminal
/// configuration.
pub fn detect_theme_mode_once() {
    let _ = DETECTED_MODE.set(theme_mode(QueryOptions::default()).unwrap_or(ThemeMode::Dark));
}

struct Ctx {
    syntaxes: SyntaxSet,
    theme: Theme,
    rust_syntax: SyntaxReference,
    asm_syntax: SyntaxReference,
}

fn ctx() -> &'static Ctx {
    static C: OnceLock<Ctx> = OnceLock::new();
    C.get_or_init(|| {
        // Start from `two-face`'s curated set (Rust + many others) and
        // augment it with our embedded SBPF grammar so the asm pane
        // colours real SBPF instead of mis-applied NASM. The grammar is
        // compiled at startup; failure to parse it is a build-time bug
        // that we surface by panicking — better than silently rendering
        // monochrome asm.
        let mut builder = two_face::syntax::extra_newlines().into_builder();
        let sbpf_def =
            SyntaxDefinition::load_from_str(SBPF_SYNTAX_YAML, true, Some("SBPF Assembly"))
                .expect("compile bundled sbpf.sublime-syntax");
        builder.add(sbpf_def);
        let syntaxes = builder.build();
        let themes = ThemeSet::load_defaults();
        // Pick a syntect theme that reads against the detected background.
        // `DETECTED_MODE` is populated by `detect_theme_mode_once()` from
        // the TUI entrypoint; if the caller forgot, we treat that as dark
        // (the more common dev-terminal default).
        let (dark_name, light_name) = ("base16-eighties.dark", "Solarized (light)");
        let theme_name = match DETECTED_MODE.get() {
            Some(ThemeMode::Light) => light_name,
            _ => dark_name,
        };
        let theme = themes
            .themes
            .get(theme_name)
            .cloned()
            .or_else(|| themes.themes.get(dark_name).cloned())
            .unwrap_or_else(|| {
                themes
                    .themes
                    .values()
                    .next()
                    .cloned()
                    .expect("default theme")
            });

        let rust_syntax = syntaxes
            .find_syntax_by_token("rs")
            .or_else(|| syntaxes.find_syntax_by_name("Rust"))
            .unwrap_or_else(|| syntaxes.find_syntax_plain_text())
            .clone();

        // Prefer our SBPF grammar; fall back to NASM only if the embedded
        // syntax somehow failed to register (would only happen if the
        // builder change above regressed).
        let asm_syntax = syntaxes
            .find_syntax_by_name("SBPF Assembly")
            .or_else(|| syntaxes.find_syntax_by_name("Assembly x86 (NASM)"))
            .or_else(|| syntaxes.find_syntax_by_token("asm"))
            .unwrap_or_else(|| syntaxes.find_syntax_plain_text())
            .clone();

        Ctx {
            syntaxes,
            theme,
            rust_syntax,
            asm_syntax,
        }
    })
}

/// Highlight one line of Rust source. Returns an owned [`Line`] of styled
/// spans suitable for handing to [`ratatui::widgets::Paragraph::new`].
///
/// Stateless per-line highlighting — multi-line strings or block comments
/// may render with the wrong colour past their first line. Acceptable for
/// the source pane's ~30-line window; `bat` uses the same trade-off when
/// rendering small slices.
pub fn highlight_rust(line: &str) -> Line<'static> {
    highlight_with(&ctx().rust_syntax, line)
}

/// Highlight one line of disassembly. Falls back to the plain-text syntax
/// if no asm grammar resolved.
pub fn highlight_asm(line: &str) -> Line<'static> {
    highlight_with(&ctx().asm_syntax, line)
}

fn highlight_with(syntax: &SyntaxReference, line: &str) -> Line<'static> {
    let c = ctx();
    let mut hl = HighlightLines::new(syntax, &c.theme);
    // syntect expects a trailing newline; appending one keeps the highlight
    // engine from breaking on lines that don't end with `\n`.
    let owned_line = if line.ends_with('\n') {
        line.to_owned()
    } else {
        format!("{line}\n")
    };
    let regions = match hl.highlight_line(&owned_line, &c.syntaxes) {
        Ok(r) => r,
        Err(_) => return Line::from(Span::raw(line.to_owned())),
    };
    let spans: Vec<Span<'static>> = regions
        .into_iter()
        .filter_map(|(style, text)| {
            // Drop the trailing `\n` we appended so the rendered line
            // doesn't include a stray newline span.
            let trimmed = text.trim_end_matches('\n');
            if trimmed.is_empty() {
                return None;
            }
            into_span((style, trimmed)).ok().map(|s| {
                // Strip the syntect theme's bg color — letting it through
                // paints a colored rectangle behind every token because the
                // theme assumes its own background, not the user's
                // terminal's. We keep the fg + modifiers so token coloring
                // still reads, with the terminal background untouched.
                let mut style = s.style;
                style.bg = None;
                Span::styled(trimmed.to_owned(), style)
            })
        })
        .collect();
    if spans.is_empty() {
        Line::from(Span::raw(line.to_owned()))
    } else {
        Line::from(spans)
    }
}
