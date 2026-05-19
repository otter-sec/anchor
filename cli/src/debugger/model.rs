//! Data model consumed by the debugger TUI.
//!
//! A [`DebugSession`] is the output of a single `anchor debugger` run,
//! holding every `(test, tx)` pair the profile callback captured. Each
//! [`DebugTx`] contains one [`DebugNode`] per program invocation (top-level
//! + CPIs) in that tx; each node owns the stream of [`DebugStep`]s it
//! executed.

use {
    ratatui::text::Span,
    std::{collections::BTreeMap, path::PathBuf},
};

/// Top-level debugger state: every trace captured across every test.
pub struct DebugSession {
    pub txs: Vec<DebugTx>,
    /// Directories to try when resolving a `SrcLoc` whose file path is
    /// relative — LLVM sometimes emits paths relative to the workspace root
    /// rather than joining them with `DW_AT_comp_dir`. Checked in order.
    pub src_roots: Vec<PathBuf>,
    /// `(prefix → replacement)` rewrites applied to absolute DWARF paths
    /// before lookup. Used to map the CI build path the local toolchain was
    /// compiled under (e.g. `/home/runner/work/platform-tools/…` on Linux,
    /// `/Users/runner/…` on macOS) to the source tree shipped with it.
    pub path_rewrites: Vec<(PathBuf, PathBuf)>,
    /// The directory the debugger was invoked from. Used by the source pane
    /// to show paths relative to the user's CWD (e.g. `src/lib.rs` instead
    /// of the full absolute path).
    pub cwd: Option<PathBuf>,
    /// Static disassembly per program, keyed by base58 program id. The
    /// instructions pane reads the current step's PC, looks up the active
    /// node's program in this map, and renders a window of PCs in memory
    /// order — so j/k stepping reveals the actual code layout instead of
    /// the chronological trace.
    pub programs: BTreeMap<String, ProgramDisasm>,
}

/// Pre-rendered static disassembly for one program. Built once at
/// arena-build time; the TUI only reads from it.
pub struct ProgramDisasm {
    /// Every traced-PC-eligible instruction in text-section order.
    pub insns: Vec<StaticInsn>,
    /// `pc → index into insns`. PCs the program never uses (data section,
    /// padding) won't be in this map; the TUI falls back to the closest
    /// preceding PC when that happens.
    pub pc_to_idx: BTreeMap<u64, usize>,
    /// True when the program's ELF carried readable DWARF line info.
    /// Lets the source pane distinguish "rebuild needed" (no DWARF
    /// anywhere) from "this PC has no source mapping" (DWARF present,
    /// but LLVM didn't emit a line entry for this PC — common for
    /// inlined frames, compiler-generated stubs, and `.text` padding).
    pub has_dwarf: bool,
}

/// One row in the static disasm view.
pub struct StaticInsn {
    pub pc: u64,
    /// Pre-highlighted spans — same syntect path as the trace cache.
    pub disasm_spans: Vec<Span<'static>>,
    /// Symbol name when this PC is a function entrypoint. Drives the
    /// "--- handler @ pc N ---" header rows in the rendered view.
    pub func_label: Option<String>,
}

/// One outer transaction's worth of traced execution.
pub struct DebugTx {
    pub test_name: String,
    /// 1-indexed tx number within its test (the `txN` from the trace filename).
    pub tx_seq: u32,
    /// Sum of `cu_cost` across all steps in all nodes.
    pub total_cu: u64,
    /// One node per invocation in call order (top-level first, then CPIs).
    pub nodes: Vec<DebugNode>,
}

/// One program invocation: either the top-level program call or a CPI.
pub struct DebugNode {
    /// Human-readable program label (e.g. `"vault_v2 (Vaul…dGpx)"` or
    /// `"[unresolved <short-pid>]"`).
    pub program_label: String,
    /// Base58 program id as written by the profile callback.
    pub program_id: String,
    pub steps: Vec<DebugStep>,
}

/// One traced SBPF instruction plus everything the TUI panes need to render
/// it.
#[derive(Clone)]
pub struct DebugStep {
    pub pc: u64,
    pub regs: [u64; 12],
    /// Raw 8-byte SBPF instruction.
    pub insn: [u8; 8],
    /// Pretty disassembly (sbpf `Analysis::disassemble_instruction`).
    pub disasm: String,
    /// Pre-highlighted disasm spans — built once at arena-build time so the
    /// instruction pane doesn't re-run syntect on every redraw. Held-down
    /// j/k stays smooth even with hundreds of visible steps because the
    /// hot path is now a clone of immutable `Span<'static>`s.
    pub disasm_spans: Vec<Span<'static>>,
    /// Resolved (short) function name containing `pc`.
    pub func: String,
    /// Call-stack depth at this step (1 = top-level function). Used for
    /// step-over/out navigation.
    pub call_depth: usize,
    /// 1 for plain insn, or `ComputeBudget` syscall base cost.
    pub cu_cost: u64,
    /// Cumulative CU consumed through (and including) this step, within its
    /// enclosing [`DebugNode`]. Handy for the status line.
    pub cu_cumulative: u64,
    /// `Some(name)` iff this is a syscall leaf step.
    pub syscall: Option<String>,
    /// Source location resolved via DWARF, if debug info was available.
    pub src_loc: Option<SrcLoc>,
}

#[derive(Clone, Debug)]
pub struct SrcLoc {
    pub file: PathBuf,
    pub line: u32,
}
