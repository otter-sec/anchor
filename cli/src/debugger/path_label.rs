//! Classify a resolved source path into a human-readable label for the
//! source pane title.
//!
//! Categories tried in order:
//!
//! 1. **Stdlib** — path lies under one of the platform-tools rewrite
//!    targets (e.g. `~/.cache/solana/v1.52/.../rust/library/`). The crate
//!    name is the first segment after `library/` (`core`, `alloc`, `std`).
//! 2. **Cargo registry crate** — path under
//!    `$CARGO_HOME/registry/src/<index>/<name>-<version>/`.
//! 3. **Cargo git checkout** — path under
//!    `$CARGO_HOME/git/checkouts/<repo>-<hash>/<rev>/...`.
//! 4. **Workspace member** — path under the workspace root. Walks up
//!    looking for the nearest `Cargo.toml` to surface the package name.
//! 5. **Unknown** — show the full path verbatim.
//!
//! Falls through to "unknown" rather than panicking on weird inputs.

use std::path::{Component, Path, PathBuf};

/// What we render in the source pane title.
#[derive(Clone, Debug)]
pub struct PathLabel {
    /// Short category label, e.g. `"stdlib · core"`, `"pinocchio v0.11.1"`,
    /// `"debugger-testing"`.
    pub label: String,
    /// Path relative to the chosen anchor (workspace root, crate root,
    /// stdlib library root). Falls back to the absolute path when no
    /// anchor matched.
    pub path_display: String,
}

/// Classify a resolved source path for the source pane title.
///
/// `cwd` is the directory the debugger was invoked from — paths under it
/// are displayed relative to it (e.g. `src/lib.rs`). `src_roots` are
/// tried next for workspace-relative display. `path_rewrites` handle
/// stdlib CI paths.
pub fn classify(
    path: &Path,
    src_roots: &[PathBuf],
    path_rewrites: &[(PathBuf, PathBuf)],
    cwd: Option<&Path>,
) -> PathLabel {
    // Highest priority: paths under the user's CWD are shown relative to
    // it with the enclosing package name as the label.
    if let Some(cwd) = cwd {
        if let Some(label) = classify_cwd(path, cwd) {
            return label;
        }
    }
    if let Some(label) = classify_stdlib(path, path_rewrites) {
        return label;
    }
    if let Some(label) = classify_cargo_registry(path) {
        return label;
    }
    if let Some(label) = classify_cargo_git(path) {
        return label;
    }
    if let Some(label) = classify_workspace(path, src_roots) {
        return label;
    }
    PathLabel {
        label: "(unknown)".to_owned(),
        path_display: path.display().to_string(),
    }
}

fn classify_cwd(path: &Path, cwd: &Path) -> Option<PathLabel> {
    let rel = path.strip_prefix(cwd).ok()?;
    let pkg = enclosing_package_name(cwd, rel);
    let label = pkg
        .or_else(|| cwd.file_name().and_then(|n| n.to_str()).map(str::to_owned))
        .unwrap_or_else(|| "workspace".to_owned());
    Some(PathLabel {
        label,
        path_display: rel.display().to_string(),
    })
}

fn classify_stdlib(path: &Path, rewrites: &[(PathBuf, PathBuf)]) -> Option<PathLabel> {
    for (_, replacement) in rewrites {
        if let Ok(rel) = path.strip_prefix(replacement) {
            // rel is e.g. `core/src/array/equality.rs`. The first segment
            // is the crate name; the rest is the in-crate path.
            let mut comps = rel.components();
            let crate_name = comps.next()?.as_os_str().to_str()?.to_owned();
            let after_crate: PathBuf = comps.collect();
            return Some(PathLabel {
                label: format!("stdlib · {crate_name}"),
                path_display: after_crate.display().to_string(),
            });
        }
    }
    None
}

fn classify_cargo_registry(path: &Path) -> Option<PathLabel> {
    let cargo_home = cargo_home()?;
    let registry = cargo_home.join("registry").join("src");
    let rel = path.strip_prefix(&registry).ok()?;
    // `rel` = `<index-hash-dir>/<name>-<version>/<rest>`
    let mut comps = rel.components();
    let _index = comps.next()?;
    let pkg_dir = comps.next()?.as_os_str().to_str()?;
    let rest: PathBuf = comps.collect();
    let (name, version) = split_name_version(pkg_dir);
    let label = match version {
        Some(v) => format!("{name} v{v}"),
        None => name.to_owned(),
    };
    Some(PathLabel {
        label,
        path_display: rest.display().to_string(),
    })
}

fn classify_cargo_git(path: &Path) -> Option<PathLabel> {
    let cargo_home = cargo_home()?;
    let checkouts = cargo_home.join("git").join("checkouts");
    let rel = path.strip_prefix(&checkouts).ok()?;
    // `rel` = `<repo>-<hash>/<rev-prefix>/<rest>`
    let mut comps = rel.components();
    let repo_dir = comps.next()?.as_os_str().to_str()?;
    let _rev = comps.next();
    let rest: PathBuf = comps.collect();
    // Strip the trailing `-<hash>` to get a readable repo name. cargo's
    // hash is 16 hex chars — when present, drop it; otherwise keep the
    // dir name verbatim.
    let repo = repo_dir
        .rsplit_once('-')
        .filter(|(_, hash)| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|(name, _)| name)
        .unwrap_or(repo_dir);
    Some(PathLabel {
        label: format!("{repo} (git)"),
        path_display: rest.display().to_string(),
    })
}

fn classify_workspace(path: &Path, src_roots: &[PathBuf]) -> Option<PathLabel> {
    for root in src_roots {
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let pkg = enclosing_package_name(root, rel);
        let label = pkg
            .or_else(|| root.file_name().and_then(|n| n.to_str()).map(str::to_owned))
            .unwrap_or_else(|| "workspace".to_owned());
        return Some(PathLabel {
            label,
            path_display: rel.display().to_string(),
        });
    }
    None
}

/// Walk up from `<workspace>/<rel>` toward `workspace` looking for a
/// `Cargo.toml`, returning the `[package] name` it declares.
fn enclosing_package_name(workspace: &Path, rel: &Path) -> Option<String> {
    let mut current = workspace.join(rel);
    while current.pop() {
        if !current.starts_with(workspace) {
            return None;
        }
        let manifest = current.join("Cargo.toml");
        if manifest.is_file() {
            return read_package_name(&manifest);
        }
    }
    None
}

/// Pull `[package] name = "..."` out of a Cargo.toml without a full TOML
/// parse — the stricter parse would just ignore everything else anyway,
/// and we don't want to drag in `cargo_toml` for a one-line lookup.
fn read_package_name(manifest: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(manifest).ok()?;
    let mut in_package = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        // `name = "foo"` or `name="foo"` — be lenient about whitespace.
        let Some(rest) = trimmed.strip_prefix("name") else {
            continue;
        };
        let after_eq = rest.trim_start().strip_prefix('=')?.trim_start();
        let stripped = after_eq.strip_prefix('"')?;
        let end = stripped.find('"')?;
        return Some(stripped[..end].to_owned());
    }
    None
}

/// Split `"<name>-<version>"` (a cargo registry dir name) into its parts.
/// Returns `(name, None)` if no parseable version suffix is present.
///
/// "Parseable" means the suffix after the last `-` starts with a digit —
/// good enough to tell `tokio-1.35.0` from `idl-1.0` (also a valid name+ver)
/// vs hyphenated-only names. We never panic on weird inputs.
fn split_name_version(dir_name: &str) -> (&str, Option<&str>) {
    if let Some((name, version)) = dir_name.rsplit_once('-') {
        if version.starts_with(|c: char| c.is_ascii_digit()) {
            return (name, Some(version));
        }
    }
    (dir_name, None)
}

fn cargo_home() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CARGO_HOME") {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join(".cargo"))
}

/// Best-effort canonicalization. Returns `path` verbatim on failure so the
/// caller doesn't have to unwrap. Used to defang `/tmp` ↔ `/private/tmp`
/// drift on macOS before prefix matching.
#[allow(dead_code)]
pub fn canonicalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Helper to detect that a single path component is "src" — used by
/// callers that want to prettify in-crate display further. Kept here so
/// the policy lives next to the other path heuristics.
#[allow(dead_code)]
pub fn is_src_dir(c: Component<'_>) -> bool {
    c.as_os_str() == "src"
}
