//! `anchor debugger` TUI.
//!
//! Two screens drive the whole experience:
//!
//! - [`Screen::Picker`] — startup. Lists every captured `(test, tx)` pair
//!   with total CU; user picks one to step into. Also reachable from the
//!   stepper via `t`, so jumping across txs within a session is one keypress.
//! - [`Screen::Stepper`] — foundry-style instruction stepper with panes for
//!   the instruction list, registers, call stack, and source.
//!
//! Keybinds mirror `forge test --debug` where it makes sense:
//!
//! ```text
//! j / k / ↑ / ↓       step ± 1 instruction
//! s / a               step over next/prev call
//! c / C               previous / next CPI invocation
//! g / G               first / last step in current node
//! t                   return to tx picker (or select in picker)
//! K / J               scroll call stack
//! q                   quit
//! 10k                 repeat count (e.g. move up 10 steps)
//! ```

use {
    super::{
        highlight::highlight_rust,
        model::{DebugSession, DebugStep, DebugTx},
        path_label::{classify, PathLabel},
    },
    crossterm::{
        event::{
            self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        },
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    ratatui::{
        backend::CrosstermBackend,
        layout::{Alignment, Constraint, Direction, Layout, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
        Frame, Terminal,
    },
    std::{collections::HashMap, io, path::PathBuf},
};

type DebugTerm = Terminal<CrosstermBackend<io::Stdout>>;

/// Run the debugger TUI over a fully-populated [`DebugSession`]. Blocks
/// until the user hits `q`.
pub fn run(session: DebugSession) -> anyhow::Result<()> {
    if session.txs.is_empty() {
        anyhow::bail!(
            "no traces to debug — did your tests call `anchor_v2_testing::svm()` and complete at \
             least one transaction?"
        );
    }

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let mut guard = TerminalGuard::new(terminal);
    let mut app = App::new(session);
    loop {
        guard.term.draw(|f| app.draw(f))?;
        // Block for the first event of the burst, then drain anything else
        // crossterm has buffered before the next redraw. Holding j/k
        // generates one event per OS keyboard-repeat tick, which used to
        // trigger one full redraw per event — now they collapse into one
        // step delta and one render. Safety cap of 256 events keeps a
        // pathological event flood from starving the redraw entirely.
        let mut flow = app.handle(event::read()?);
        let mut drained = 0;
        while flow == Flow::Continue && drained < 256 && event::poll(std::time::Duration::ZERO)? {
            flow = app.handle(event::read()?);
            drained += 1;
        }
        if flow == Flow::Quit {
            break;
        }
    }
    Ok(())
}

enum Screen {
    Picker,
    Stepper,
}

#[derive(PartialEq, Eq)]
enum Flow {
    Continue,
    Quit,
}

struct App {
    session: DebugSession,
    screen: Screen,
    /// Picker list state.
    picker: ListState,
    /// Index into `session.txs`.
    current_tx: usize,
    /// Index into `session.txs[current_tx].nodes` — the active call context
    /// (0 = top-level, 1.. = CPIs in order).
    current_node: usize,
    /// Index into the active node's `steps`.
    current_step: usize,
    /// Digit buffer for `10k`-style repeats.
    key_buffer: String,
    /// File contents cache. Source files are read on first access (path
    /// resolution + disk read) and kept for the session — frame-rate
    /// stepping was bottlenecked on `read_to_string` per redraw.
    file_cache: HashMap<PathBuf, FileEntry>,
    /// Per-(file, line) highlighted-span cache. Avoids re-running syntect
    /// on the same source line every redraw while you hold j/k.
    highlight_cache: HashMap<(PathBuf, u32), Vec<Span<'static>>>,
    /// Per-path label cache (crate / stdlib / workspace classification).
    /// `classify` walks Cargo.toml siblings on workspace files, so we only
    /// want to do that once per file across the whole session.
    label_cache: HashMap<PathBuf, PathLabel>,
    /// Pre-baked picker rows: a flat sequence of `Header(test_name)` /
    /// `Tx(tx_idx)` entries, sorted so each test's children sit under it.
    /// Built once at App-init from the (test, tx)-sorted `session.txs`.
    picker_rows: Vec<PickerRow>,
}

#[derive(Clone)]
enum PickerRow {
    /// Test name banner — non-selectable. Skipped by j/k navigation.
    Header(String),
    /// Selectable row pointing back into `session.txs`.
    Tx(usize),
}

/// One entry in [`App::file_cache`]: either the file's lines, or the error
/// we hit reading it (so we don't retry the disk every frame for missing
/// stdlib paths).
enum FileEntry {
    Loaded(Vec<String>),
    Missing(String),
}

impl App {
    fn new(session: DebugSession) -> Self {
        // Build the grouped row list once: emit a header row whenever the
        // test_name changes, then a Tx row per tx in that group. Picker
        // selection navigates this list with `next_selectable_*` to skip
        // header rows so the user never lands on one.
        let mut picker_rows: Vec<PickerRow> = Vec::with_capacity(session.txs.len() * 2);
        let mut last_test: Option<&str> = None;
        for (i, tx) in session.txs.iter().enumerate() {
            if last_test != Some(tx.test_name.as_str()) {
                picker_rows.push(PickerRow::Header(tx.test_name.clone()));
                last_test = Some(tx.test_name.as_str());
            }
            picker_rows.push(PickerRow::Tx(i));
        }
        let initial = picker_rows
            .iter()
            .position(|r| matches!(r, PickerRow::Tx(_)))
            .unwrap_or(0);

        let mut picker = ListState::default();
        picker.select(Some(initial));
        Self {
            session,
            screen: Screen::Picker,
            picker,
            current_tx: 0,
            current_node: 0,
            current_step: 0,
            key_buffer: String::new(),
            file_cache: HashMap::new(),
            highlight_cache: HashMap::new(),
            label_cache: HashMap::new(),
            picker_rows,
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>) {
        match self.screen {
            Screen::Picker => self.draw_picker(f),
            Screen::Stepper => self.draw_stepper(f),
        }
    }

    fn handle(&mut self, ev: Event) -> Flow {
        match (ev, &self.screen) {
            (Event::Key(k), Screen::Picker) => self.handle_picker_key(k),
            (Event::Key(k), Screen::Stepper) => self.handle_stepper_key(k),
            _ => Flow::Continue,
        }
    }

    // --- picker --------------------------------------------------------------

    fn draw_picker(&mut self, f: &mut Frame<'_>) {
        let area = f.area();
        let [title, list, footer] = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(3),
                Constraint::Min(3),
                Constraint::Length(3),
            ],
        )
        .areas(area);

        let title_block = Paragraph::new(Line::from(vec![
            Span::styled("anchor debugger", Style::new().add_modifier(Modifier::BOLD)),
            Span::raw(format!("  —  {} transaction(s)", self.session.txs.len())),
        ]))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(title_block, title);

        let test_count = self
            .picker_rows
            .iter()
            .filter(|r| matches!(r, PickerRow::Header(_)))
            .count();
        let items: Vec<ListItem> = self
            .picker_rows
            .iter()
            .map(|row| match row {
                PickerRow::Header(name) => {
                    // Compact header: indented less than the tx rows so it
                    // visually sits "above" them. Dim by default so the
                    // selected tx still pops.
                    ListItem::new(Line::from(vec![Span::styled(
                        format!("{name}"),
                        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )]))
                    // Disable selection-highlight on header rows so j/k
                    // visually skips them even mid-frame; the
                    // `next_selectable_*` helpers do the actual skipping.
                    .style(Style::new())
                }
                PickerRow::Tx(idx) => {
                    let tx = &self.session.txs[*idx];
                    let top = tx
                        .nodes
                        .first()
                        .map(|n| n.program_label.as_str())
                        .unwrap_or("");
                    let cpis = tx.nodes.len().saturating_sub(1);
                    let cpi_badge = if cpis > 0 {
                        format!("  +{cpis} CPI")
                    } else {
                        String::new()
                    };
                    ListItem::new(Line::from(vec![
                        // Tree-style indent so the parent test reads as a
                        // group header.
                        Span::styled("  ├─ ", Style::new().fg(Color::DarkGray)),
                        Span::styled(
                            format!("tx{:<3}", tx.tx_seq),
                            Style::new().fg(Color::Yellow),
                        ),
                        Span::raw(format!("  {:>8} CU  ", tx.total_cu)),
                        Span::raw(top.to_string()),
                        Span::styled(cpi_badge, Style::new().fg(Color::Magenta)),
                    ]))
                }
            })
            .collect();

        let title = format!(
            " {} test(s), {} tx(s) — select one to step into ",
            test_count,
            self.session.txs.len()
        );
        let list_widget = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(
                Style::new()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");
        f.render_stateful_widget(list_widget, list, &mut self.picker);

        let help = Paragraph::new("j/k or ↑/↓  select    enter/t  open    q  quit")
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(help, footer);
    }

    fn handle_picker_key(&mut self, k: KeyEvent) -> Flow {
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('j') | KeyCode::Down => self.picker_next(),
            KeyCode::Char('k') | KeyCode::Up => self.picker_prev(),
            KeyCode::Char('g') | KeyCode::Home => {
                self.picker.select(self.first_selectable());
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.picker.select(self.last_selectable());
            }
            KeyCode::Enter | KeyCode::Char('t') | KeyCode::Char('l') | KeyCode::Right => {
                self.open_selected();
            }
            _ => {}
        }
        Flow::Continue
    }

    /// Step the picker down to the next `Tx` row, skipping headers.
    /// Stops at the last selectable row instead of wrapping.
    fn picker_next(&mut self) {
        let from = self.picker.selected().unwrap_or(0);
        let next = self
            .picker_rows
            .iter()
            .enumerate()
            .skip(from + 1)
            .find(|(_, r)| matches!(r, PickerRow::Tx(_)))
            .map(|(i, _)| i)
            .unwrap_or(from);
        self.picker.select(Some(next));
    }

    fn picker_prev(&mut self) {
        let from = self.picker.selected().unwrap_or(0);
        let next = self
            .picker_rows
            .iter()
            .enumerate()
            .take(from)
            .rev()
            .find(|(_, r)| matches!(r, PickerRow::Tx(_)))
            .map(|(i, _)| i)
            .unwrap_or(from);
        self.picker.select(Some(next));
    }

    fn first_selectable(&self) -> Option<usize> {
        self.picker_rows
            .iter()
            .position(|r| matches!(r, PickerRow::Tx(_)))
    }

    fn last_selectable(&self) -> Option<usize> {
        self.picker_rows
            .iter()
            .rposition(|r| matches!(r, PickerRow::Tx(_)))
    }

    fn open_selected(&mut self) {
        let Some(i) = self.picker.selected() else {
            return;
        };
        if let Some(PickerRow::Tx(tx_idx)) = self.picker_rows.get(i) {
            self.current_tx = *tx_idx;
            self.current_node = 0;
            self.current_step = 0;
            self.screen = Screen::Stepper;
        }
    }

    // --- stepper -------------------------------------------------------------

    fn current_tx(&self) -> &DebugTx {
        &self.session.txs[self.current_tx]
    }

    fn current_steps(&self) -> &[DebugStep] {
        &self.current_tx().nodes[self.current_node].steps
    }

    fn draw_stepper(&mut self, f: &mut Frame<'_>) {
        // (existing body — `&mut self` already in scope so the source pane
        // can write through to `file_cache` / `highlight_cache`.)
        let area = f.area();
        if area.width < 80 || area.height < 20 {
            let msg = Paragraph::new(format!(
                "terminal too small ({}x{}) — need at least 80x20",
                area.width, area.height
            ))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
            f.render_widget(msg, area);
            return;
        }

        // Header (title + invocation breadcrumb), main region, footer.
        let [header, main, footer] = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(3),
            ],
        )
        .areas(area);

        self.draw_stepper_header(f, header);

        // Main: left = instructions, right = (regs over source).
        let [left, right] = Layout::new(
            Direction::Horizontal,
            [Constraint::Percentage(55), Constraint::Percentage(45)],
        )
        .areas(main);

        self.draw_instructions(f, left);

        let [right_top, right_bot] = Layout::new(
            Direction::Vertical,
            [Constraint::Length(14), Constraint::Min(5)],
        )
        .areas(right);
        self.draw_registers(f, right_top);
        self.draw_source(f, right_bot);

        let footer_text = Paragraph::new(
            "j/k step   s/a step-over   c/C prev/next CPI   g/G first/last   t tx picker   q quit",
        )
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
        f.render_widget(footer_text, footer);
    }

    fn draw_stepper_header(&self, f: &mut Frame<'_>, area: Rect) {
        let tx = self.current_tx();
        let node = &tx.nodes[self.current_node];
        let n_nodes = tx.nodes.len();
        let step_total = node.steps.len();

        // Line 1: test/tx breadcrumb + step + cu.
        let title_line = Line::from(vec![
            Span::styled(
                format!("{} ", tx.test_name),
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("· tx{} ", tx.tx_seq)),
            Span::styled(
                format!(
                    "· step {}/{}  cu {}",
                    self.current_step + 1,
                    step_total.max(1),
                    node.steps
                        .get(self.current_step)
                        .map(|s| s.cu_cumulative)
                        .unwrap_or(0)
                ),
                Style::new().fg(Color::Yellow),
            ),
        ]);

        // Line 2: invocation breadcrumb. Renders all nodes with the
        // current one inverted so c/C navigation is obvious even when
        // every node is the same program (the self-CPI case).
        let invocations_line = if n_nodes <= 1 {
            Line::from(vec![Span::styled(
                format!(
                    "invocations: 1/1 ({})  — single invocation, c/C disabled",
                    node.program_label
                ),
                Style::new().fg(Color::DarkGray),
            )])
        } else {
            // Inactive items render in the default foreground (terminal's
            // own white-ish), active item is inverted so it pops without
            // imposing a hard-coded color the user's theme didn't pick.
            // Separators stay dim so the eye lands on the items themselves.
            let mut spans: Vec<Span<'static>> = vec![Span::styled(
                "invocations: ",
                Style::new().fg(Color::DarkGray),
            )];
            for (i, n) in tx.nodes.iter().enumerate() {
                let is_cur = i == self.current_node;
                let kind = if i == 0 { "top" } else { "cpi" };
                let label = format!(" #{} {} {} ", i + 1, kind, n.program_label);
                let style = if is_cur {
                    Style::new().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else {
                    Style::new()
                };
                spans.push(Span::styled(label, style));
                if i + 1 < tx.nodes.len() {
                    spans.push(Span::styled(" → ", Style::new().fg(Color::DarkGray)));
                }
            }
            Line::from(spans)
        };

        let header = Paragraph::new(vec![title_line, invocations_line])
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(header, area);
    }

    fn draw_instructions(&mut self, f: &mut Frame<'_>, area: Rect) {
        let tx = self.current_tx();
        let node = &tx.nodes[self.current_node];
        let Some(step) = node.steps.get(self.current_step) else {
            f.render_widget(
                Paragraph::new("(no steps)").block(
                    Block::default()
                        .title(" instructions ")
                        .borders(Borders::ALL),
                ),
                area,
            );
            return;
        };

        // Static-disasm view: render the program's text section in PC
        // order, centered on the current step's PC. j/k stepping over a
        // call/branch jumps the PC; the view re-centers each frame so
        // you always see the actual code layout around what executed.
        if let Some(disasm) = self.session.programs.get(&node.program_id) {
            self.draw_static_disasm(f, area, node, step, disasm);
            return;
        }

        // Fallback: program ELF wasn't resolvable (e.g. third-party
        // deploy) so we have no static disasm. Drop back to the trace
        // stream view — same data the flamegraph consumes.
        self.draw_trace_stream(f, area);
    }

    fn draw_static_disasm(
        &self,
        f: &mut Frame<'_>,
        area: Rect,
        node: &super::model::DebugNode,
        step: &DebugStep,
        disasm: &super::model::ProgramDisasm,
    ) {
        // Locate the current PC in the static index. Falls back to the
        // nearest preceding PC if the exact one isn't there (shouldn't
        // happen for in-text PCs, but we never want to panic on a weird
        // trace).
        let center_idx = disasm
            .pc_to_idx
            .get(&step.pc)
            .copied()
            .or_else(|| {
                disasm
                    .pc_to_idx
                    .range(..=step.pc)
                    .next_back()
                    .map(|(_, i)| *i)
            })
            .unwrap_or(0);

        let window = area.height.saturating_sub(2) as usize;
        let half = window / 2;
        let start = center_idx.saturating_sub(half);
        let end = (start + window).min(disasm.insns.len());

        let mut rows: Vec<ListItem> = Vec::with_capacity(end - start);
        for insn in &disasm.insns[start..end] {
            // Symbol header above the function entrypoint, when known.
            // Costs one row per visible function boundary inside the
            // window — rare enough to ignore for sizing.
            if let Some(label) = &insn.func_label {
                rows.push(ListItem::new(Line::from(vec![
                    Span::styled("   ", Style::new()),
                    Span::styled(
                        format!("┌── {label}"),
                        Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    ),
                ])));
            }

            let is_current = insn.pc == step.pc;
            let marker = if is_current { ">" } else { " " };
            let mut spans = vec![
                Span::raw(format!("{marker} ")),
                Span::styled(
                    format!("pc {:>5} ", insn.pc),
                    Style::new().fg(Color::DarkGray),
                ),
            ];
            spans.extend(insn.disasm_spans.iter().cloned());
            let line = Line::from(spans);
            let style = if is_current {
                Style::new()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };
            rows.push(ListItem::new(line).style(style));
        }

        let title = format!(
            " {} · pc {} · trace step {}/{} ",
            node.program_label,
            step.pc,
            self.current_step + 1,
            node.steps.len()
        );
        let widget = List::new(rows).block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(widget, area);
    }

    /// Fallback for unresolved programs: chronological trace view.
    fn draw_trace_stream(&self, f: &mut Frame<'_>, area: Rect) {
        let steps = self.current_steps();
        let window = area.height.saturating_sub(2) as usize;
        let half = window / 2;
        let start = self.current_step.saturating_sub(half);
        let end = (start + window).min(steps.len());

        let items: Vec<ListItem> = steps[start..end]
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let idx = start + i;
                let marker = if idx == self.current_step { ">" } else { " " };
                let mut spans = vec![
                    Span::raw(format!("{marker} ")),
                    Span::styled(format!("{:>6} ", idx), Style::new().fg(Color::DarkGray)),
                    Span::styled(format!("pc {:>5} ", s.pc), Style::new().fg(Color::DarkGray)),
                ];
                spans.extend(s.disasm_spans.iter().cloned());
                let line = Line::from(spans);
                let style = if idx == self.current_step {
                    Style::new()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                };
                ListItem::new(line).style(style)
            })
            .collect();

        let title = format!(
            " trace stream — no static disasm  ({} / {}) ",
            self.current_step + 1,
            steps.len()
        );
        let widget = List::new(items).block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(widget, area);
    }

    fn draw_registers(&self, f: &mut Frame<'_>, area: Rect) {
        let Some(step) = self.current_steps().get(self.current_step) else {
            let widget = Paragraph::new("(no steps)")
                .block(Block::default().title(" registers ").borders(Borders::ALL));
            f.render_widget(widget, area);
            return;
        };

        let prev = self
            .current_step
            .checked_sub(1)
            .and_then(|i| self.current_steps().get(i));

        let mut lines = Vec::with_capacity(12);
        for r in 0..11 {
            let val = step.regs[r];
            let changed = prev.map_or(false, |p| p.regs[r] != val);
            let style = if changed {
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::new()
            };
            lines.push(Line::from(vec![
                Span::raw(format!("r{:<2} ", r)),
                Span::styled(format!("{:#018x}", val), style),
                Span::styled(
                    format!("  ({})", val as i64),
                    Style::new().fg(Color::DarkGray),
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::raw("pc  "),
            Span::styled(
                format!("{:#010x}  ({})", step.pc, step.pc),
                Style::new().fg(Color::Cyan),
            ),
        ]));

        let widget = Paragraph::new(lines)
            .block(Block::default().title(" registers ").borders(Borders::ALL));
        f.render_widget(widget, area);
    }

    fn draw_source(&mut self, f: &mut Frame<'_>, area: Rect) {
        let block = Block::default().title(" source ").borders(Borders::ALL);
        let Some(step) = self.current_steps().get(self.current_step).cloned() else {
            f.render_widget(Paragraph::new("(no steps)").block(block), area);
            return;
        };
        let Some(loc) = step.src_loc.clone() else {
            // Two distinct failure modes — distinguishing them keeps us
            // from telling users to rebuild a binary that's already
            // built with debug info.
            let node = &self.current_tx().nodes[self.current_node];
            let program_disasm = self.session.programs.get(&node.program_id);
            let msg = match program_disasm {
                Some(d) if d.has_dwarf => format!(
                    "pc {pc:#x} has no DWARF line entry.\n\nThis is normal for hand-written asm \
                     entrypoints, inlined\nframes, compiler-generated stubs, and `.text` padding \
                     —\nLLVM only emits (file, line) tuples for Rust source.\nOther PCs in \
                     {program} resolve fine; stepping forward\nshould re-enter mapped code.\n\nIf \
                     you wrote an asm entrypoint, you can ignore this for\nthe asm region — \
                     registers + disasm above stay live.",
                    pc = step.pc,
                    program = node.program_label,
                ),
                Some(_) => format!(
                    "{program}: ELF has no DWARF line info.\n\nRebuild with debug info:\n  \
                     CARGO_PROFILE_RELEASE_DEBUG=2 anchor build --no-idl\n\n(`anchor debugger` \
                     sets this for you when it rebuilds.\nThe flag only sticks if the .so we read \
                     came from that build.)",
                    program = node.program_label,
                ),
                None => format!(
                    "no static disasm or DWARF for {program} (pc {pc:#x}).\n\nThe program's \
                     deployed `.so` wasn't resolvable from the\nworkspace's Anchor.toml — \
                     third-party deploy, or\nmismatched program-id mapping.",
                    program = node.program_label,
                    pc = step.pc,
                ),
            };
            f.render_widget(
                Paragraph::new(msg).block(block).wrap(Wrap { trim: true }),
                area,
            );
            return;
        };

        let resolved_path = resolve_src_path(
            &loc.file,
            &self.session.src_roots,
            &self.session.path_rewrites,
            loc.line,
        );
        let Some(path) = resolved_path else {
            let msg = format!(
                "can't read {}:{}\nno candidate path resolved\n\ntried roots: {}",
                loc.file.display(),
                loc.line,
                self.session
                    .src_roots
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            f.render_widget(
                Paragraph::new(msg).block(block).wrap(Wrap { trim: true }),
                area,
            );
            return;
        };

        // Pull file contents from cache, populating on first access. Errors
        // are cached too so we don't retry the disk every frame for paths
        // that resolved but can't be read (rare — usually a permissions
        // issue).
        let file_lines = match self.load_file(&path) {
            Ok(lines) => lines,
            Err(e) => {
                let msg = format!("can't read {}:{}\n{e}", path.display(), loc.line);
                f.render_widget(
                    Paragraph::new(msg).block(block).wrap(Wrap { trim: true }),
                    area,
                );
                return;
            }
        };
        let lines = window_from_lines(file_lines, loc.line, area.height.saturating_sub(2) as u32);

        // Pull the cached label or compute it once. Title format:
        //   " stdlib · core · src/array/equality.rs:150 "
        //   " pinocchio v0.11.1 · src/cpi.rs:42 "
        //   " debugger-testing · programs/debugger-testing/src/lib.rs:12 "
        let label = if let Some(cached) = self.label_cache.get(&path) {
            cached.clone()
        } else {
            let l = classify(
                &path,
                &self.session.src_roots,
                &self.session.path_rewrites,
                self.session.cwd.as_deref(),
            );
            self.label_cache.insert(path.clone(), l.clone());
            l
        };
        let title = format!(" {} · {}:{} ", label.label, label.path_display, loc.line);
        let is_rust = loc
            .file
            .extension()
            .and_then(|s| s.to_str())
            .map_or(false, |e| e.eq_ignore_ascii_case("rs"));
        let text: Vec<Line> = lines
            .into_iter()
            .map(|(n, content, is_current)| {
                let mut spans = vec![Span::styled(
                    format!("{n:>5}  "),
                    Style::new().fg(Color::DarkGray),
                )];
                if is_rust {
                    // Per-line highlight cache keyed by (resolved-path,
                    // line-number). Means a held-down j/k that scrolls
                    // through a 30-line window only ever pays for the
                    // first time each line is shown.
                    let key = (path.clone(), n);
                    let highlighted: Vec<Span<'static>> =
                        if let Some(cached) = self.highlight_cache.get(&key) {
                            cached.clone()
                        } else {
                            let h = highlight_rust(&content).spans;
                            self.highlight_cache.insert(key, h.clone());
                            h
                        };
                    if is_current {
                        for span in highlighted {
                            let mut style = span.style;
                            style.bg = Some(Color::DarkGray);
                            style = style.add_modifier(Modifier::BOLD);
                            spans.push(Span::styled(span.content.into_owned(), style));
                        }
                    } else {
                        spans.extend(highlighted);
                    }
                } else {
                    let style = if is_current {
                        Style::new()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::new()
                    };
                    spans.push(Span::styled(content, style));
                }
                Line::from(spans)
            })
            .collect();

        let widget =
            Paragraph::new(text).block(Block::default().title(title).borders(Borders::ALL));
        f.render_widget(widget, area);
    }

    fn handle_stepper_key(&mut self, k: KeyEvent) -> Flow {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Char('q') => return Flow::Quit,
            KeyCode::Char('t') | KeyCode::Esc => {
                // Map back from `current_tx` (index into session.txs) to
                // the picker row that points at it, so the user lands on
                // the same entry they entered through.
                let row = self
                    .picker_rows
                    .iter()
                    .position(|r| matches!(r, PickerRow::Tx(i) if *i == self.current_tx));
                self.picker.select(row);
                self.screen = Screen::Picker;
            }
            KeyCode::Char(d @ '0'..='9') => {
                self.key_buffer.push(d);
                return Flow::Continue;
            }
            KeyCode::Char('j') | KeyCode::Down => self.repeat(|app| app.step_forward(1)),
            KeyCode::Char('k') | KeyCode::Up => self.repeat(|app| app.step_back(1)),
            KeyCode::Char('s') => self.repeat(App::step_over_forward),
            KeyCode::Char('a') => self.repeat(App::step_over_back),
            KeyCode::Char('g') => self.current_step = 0,
            KeyCode::Char('G') => {
                self.current_step = self.current_steps().len().saturating_sub(1);
            }
            KeyCode::Char('c') if !ctrl => {
                self.current_node = self.current_node.saturating_sub(1);
                self.current_step = 0;
            }
            KeyCode::Char('C') => {
                let max = self.current_tx().nodes.len().saturating_sub(1);
                self.current_node = (self.current_node + 1).min(max);
                self.current_step = 0;
            }
            _ => {}
        }
        self.key_buffer.clear();
        Flow::Continue
    }

    fn repeat(&mut self, mut f: impl FnMut(&mut Self)) {
        let n = self
            .key_buffer
            .parse::<usize>()
            .unwrap_or(1)
            .clamp(1, 100_000);
        for _ in 0..n {
            f(self);
        }
    }

    fn step_forward(&mut self, n: usize) {
        let last = self.current_steps().len().saturating_sub(1);
        self.current_step = (self.current_step + n).min(last);
    }

    fn step_back(&mut self, n: usize) {
        self.current_step = self.current_step.saturating_sub(n);
    }

    /// Step-over: advance until call_depth returns to the current level (or
    /// shallower). Falls back to a single step when there's no nested frame.
    fn step_over_forward(&mut self) {
        let steps = self.current_steps();
        let Some(cur) = steps.get(self.current_step) else {
            return;
        };
        let base = cur.call_depth;
        let start = self.current_step + 1;
        let idx = steps[start..]
            .iter()
            .position(|s| s.call_depth <= base)
            .map(|off| start + off)
            .unwrap_or_else(|| steps.len().saturating_sub(1));
        self.current_step = idx;
    }

    fn step_over_back(&mut self) {
        let steps = self.current_steps();
        let Some(cur) = steps.get(self.current_step) else {
            return;
        };
        let base = cur.call_depth;
        let idx = steps[..self.current_step]
            .iter()
            .rposition(|s| s.call_depth <= base)
            .unwrap_or(0);
        self.current_step = idx;
    }
}

struct TerminalGuard {
    term: DebugTerm,
}

impl TerminalGuard {
    fn new(mut term: DebugTerm) -> Self {
        let _ = enable_raw_mode();
        let _ = execute!(term.backend_mut(), EnterAlternateScreen, EnableMouseCapture);
        let _ = term.hide_cursor();
        let _ = term.clear();
        Self { term }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.term.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.term.show_cursor();
    }
}

impl App {
    /// Read a source file from the cache, populating it on first miss.
    /// Returns the file's lines verbatim. Errors are cached as
    /// `Missing(msg)` so we don't hit the disk repeatedly for paths that
    /// resolved but failed to read.
    fn load_file(&mut self, path: &std::path::Path) -> Result<&[String], String> {
        if !self.file_cache.contains_key(path) {
            let entry = match std::fs::read_to_string(path) {
                Ok(s) => FileEntry::Loaded(s.lines().map(str::to_owned).collect()),
                Err(e) => FileEntry::Missing(e.to_string()),
            };
            self.file_cache.insert(path.to_path_buf(), entry);
        }
        match self.file_cache.get(path).expect("just inserted") {
            FileEntry::Loaded(lines) => Ok(lines.as_slice()),
            FileEntry::Missing(msg) => Err(msg.clone()),
        }
    }
}

/// Slice a centered window around `target_line` from already-loaded lines.
/// Returns `(line_number, content, is_current)` triples for the source
/// pane to render — does no I/O.
fn window_from_lines(lines: &[String], target_line: u32, height: u32) -> Vec<(u32, String, bool)> {
    let target_idx = target_line.saturating_sub(1) as usize;
    let half = (height / 2) as usize;
    let start = target_idx.saturating_sub(half).min(lines.len());
    let end = (start + height as usize).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let n = (start + i + 1) as u32;
            (n, l.clone(), n == target_line)
        })
        .collect()
}

/// Resolves a DWARF-reported source path to one we can actually read.
///
/// Tried in order:
/// 1. The path as-is if it's absolute and exists (local build).
/// 2. Prefix rewrites — used for stdlib frames whose DWARF path points at
///    the CI machine that built `platform-tools`.
/// 3. Join against each configured source root (workspace root etc.) for
///    paths DWARF left relative.
///
/// Returns `None` when nothing hits. The caller surfaces that as a
/// "source not available" notice in the TUI source pane.
fn resolve_src_path(
    file: &std::path::Path,
    roots: &[std::path::PathBuf],
    rewrites: &[(std::path::PathBuf, std::path::PathBuf)],
    line: u32,
) -> Option<std::path::PathBuf> {
    if file.is_absolute() && file.exists() {
        return Some(file.to_path_buf());
    }

    // Prefix rewrites (e.g. `platform-tools` CI path → local stdlib cache).
    if let Some(file_str) = file.to_str() {
        for (prefix, replacement) in rewrites {
            if let Some(prefix_str) = prefix.to_str() {
                if let Some(tail) = file_str.strip_prefix(prefix_str) {
                    let candidate = replacement.join(tail.trim_start_matches('/'));
                    if candidate.exists() {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    // SBF DWARF omits DW_AT_comp_dir (-Zremap-cwd-prefix= strips it),
    // so relative paths like `src/lib.rs` are ambiguous across crates.
    // When multiple roots contain a matching file, prefer the one that
    // actually has enough lines for the DWARF-referenced line number.
    // This correctly disambiguates e.g. anchor-lang-v2's `src/cpi.rs`
    // (538 lines) from pinocchio's `src/cpi.rs` (683 lines) when the
    // DWARF says line 668.
    let mut fallback: Option<std::path::PathBuf> = None;
    for root in roots {
        let candidate = root.join(file);
        if candidate.exists() {
            if line == 0 {
                return Some(candidate);
            }
            if let Ok(contents) = std::fs::read(&candidate) {
                let line_count = contents.iter().filter(|&&b| b == b'\n').count() + 1;
                if line as usize <= line_count {
                    return Some(candidate);
                }
            }
            if fallback.is_none() {
                fallback = Some(candidate);
            }
        }
    }

    if let Some(fb) = fallback {
        return Some(fb);
    }

    if file.exists() {
        return Some(file.to_path_buf());
    }
    None
}
