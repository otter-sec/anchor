//! Walks the profile trace directory and materializes the [`DebugSession`]
//! the TUI consumes.
//!
//! Shares the core PC-driven call-stack algorithm with the flamegraph
//! renderer via [`crate::flamegraph::trace::stream_trace`]. The two
//! consumers differ only in their sink: the SVG folder aggregates into
//! flat `(stack → cu)` maps, and this module emits one [`DebugStep`] per
//! traced instruction plus DWARF-resolved source locations.

use {
    super::{
        cargo_deps::{discover_dep_src_roots, discover_path_dep_roots},
        highlight::highlight_asm,
        model::{DebugNode, DebugSession, DebugStep, DebugTx, ProgramDisasm, StaticInsn},
        source::{discover_platform_tools_stdlib_roots, SourceResolver, CI_PLATFORM_TOOLS_PREFIX},
    },
    crate::flamegraph::trace::{
        discover_invocations, find_unstripped_binary, load_function_map, stream_trace,
        InvocationFiles, INSN_ENTRY_SIZE, KNOWN_SYSCALLS, REGS_ENTRY_SIZE,
    },
    anyhow::{anyhow, bail, Context, Result},
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_sbpf::{ebpf, static_analysis::Analysis},
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        sync::Arc,
    },
};

/// Build the session from `<profile_dir>/<test_name>/` trace directories.
///
/// `programs` maps base58 program_id → deployed `.so` path. `manifest_dir`
/// points the unstripped-binary search toward the right workspace root (same
/// convention as [`crate::profile::render_all_tests`]).
pub fn build_session(
    profile_dir: &Path,
    programs: &BTreeMap<String, PathBuf>,
    manifest_dir: Option<&Path>,
    crate_dir: Option<&Path>,
    test_filter: Option<&str>,
) -> Result<DebugSession> {
    let mut txs: Vec<DebugTx> = Vec::new();
    let mut src_roots: Vec<PathBuf> = Vec::new();
    // Path deps (e.g. `anchor-lang-v2 = { path = "../../../../lang-v2" }`)
    // must come before the workspace root so their `src/lib.rs` wins over
    // a same-named file at the workspace root (`bench/src/lib.rs`). SBF
    // DWARF omits `DW_AT_comp_dir`, so relative paths like `src/lib.rs`
    // are ambiguous — insertion order is the only disambiguation.
    if let Some(c) = crate_dir {
        src_roots.extend(discover_path_dep_roots(c));
    }
    if let Some(p) = manifest_dir {
        src_roots.push(p.to_path_buf());
        src_roots.extend(discover_dep_src_roots(p));
    }

    // Rewrite the baked-in CI platform-tools prefix on stdlib frames to the
    // local toolchain's bundled Rust source. One entry per installed
    // platform-tools version, newest first — `resolve_src_path` picks the
    // first rewrite whose resulting path exists.
    let path_rewrites: Vec<(PathBuf, PathBuf)> = discover_platform_tools_stdlib_roots()
        .into_iter()
        .map(|replacement| (PathBuf::from(CI_PLATFORM_TOOLS_PREFIX), replacement))
        .collect();

    let cwd = crate_dir.map(|p| p.to_path_buf());

    if !profile_dir.exists() {
        return Ok(DebugSession {
            txs,
            src_roots,
            path_rewrites,
            cwd,
            programs: BTreeMap::new(),
        });
    }

    // Per-program context: symbol map + syscall hashes + parsed executable +
    // text bytes + DWARF resolver. Loaded once and reused across every
    // invocation of the same program, since parsing an SBPF ELF is expensive.
    let mut program_ctx: BTreeMap<String, ProgramCtx> = BTreeMap::new();

    for entry in fs::read_dir(profile_dir)
        .with_context(|| format!("read profile dir {}", profile_dir.display()))?
    {
        let entry = entry?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(test_name) = dir.file_name().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        if let Some(filter) = test_filter {
            if !test_name.contains(filter) {
                continue;
            }
        }

        let invocations = discover_invocations(&dir)
            .with_context(|| format!("discover invocations in {}", dir.display()))?;
        if invocations.is_empty() {
            continue;
        }

        let mut tx_groups: BTreeMap<u32, Vec<&InvocationFiles>> = BTreeMap::new();
        for inv in &invocations {
            tx_groups.entry(inv.tx_seq).or_default().push(inv);
        }

        for (tx_seq, mut invs) in tx_groups {
            // `iterate_vm_traces` (in v2-testing's profile callback) walks
            // invocations bottom-up: deepest CPI first, top-level last. We
            // want call order — top-level → CPI children — so the
            // breadcrumb's "#1 top" matches what the user actually sent.
            // Reverse here; inv_seq stays as written by the callback so
            // filenames keep their stable ordering.
            invs.reverse();

            let mut nodes: Vec<DebugNode> = Vec::with_capacity(invs.len());
            let mut tx_total_cu: u64 = 0;

            for inv in invs {
                let ctx = match load_program_ctx(
                    &inv.program_id,
                    programs,
                    manifest_dir,
                    &mut program_ctx,
                ) {
                    Some(c) => c,
                    None => continue,
                };

                let regular_regs = fs::read(&inv.regs_path)
                    .with_context(|| format!("read {}", inv.regs_path.display()))?;
                let regular_insns = fs::read(&inv.insns_path)
                    .with_context(|| format!("read {}", inv.insns_path.display()))?;
                let regular_count = (regular_regs.len() / REGS_ENTRY_SIZE)
                    .min(regular_insns.len() / INSN_ENTRY_SIZE);

                let (regs, insns, exact_cu) =
                    match load_gdb_sidecars(inv, &regular_regs, &regular_insns, regular_count)? {
                        Some(sidecar) => (sidecar.regs, sidecar.insns, Some(sidecar.cu_costs)),
                        None => (regular_regs, regular_insns, None),
                    };

                let count = (regs.len() / REGS_ENTRY_SIZE).min(insns.len() / INSN_ENTRY_SIZE);
                if count == 0 {
                    continue;
                }

                let program_label = program_label(&inv.program_id, programs);
                let mut steps: Vec<DebugStep> = Vec::with_capacity(count);
                let mut node_cu: u64 = 0;

                let budget = ComputeBudget::new_with_defaults(false, false);
                let mut step_idx = 0usize;

                stream_trace(
                    &regs,
                    &insns,
                    count,
                    &ctx.symbols,
                    &ctx.syscalls,
                    &program_label,
                    &budget,
                    |s| {
                        let cu_cost = exact_cu
                            .as_ref()
                            .and_then(|costs| costs.get(step_idx).copied())
                            .unwrap_or(s.cu_cost);
                        step_idx += 1;
                        node_cu += cu_cost;
                        let disasm = disassemble(&ctx.analysis, &s.insn, s.pc as usize);
                        // Pre-highlight here so the redraw hot path is a
                        // simple span clone — see `DebugStep.disasm_spans`
                        // for the perf rationale.
                        let disasm_spans = highlight_asm(&disasm).spans;
                        let src_loc = ctx.source.resolve(s.pc);
                        steps.push(DebugStep {
                            pc: s.pc,
                            regs: s.regs,
                            insn: s.insn,
                            disasm,
                            disasm_spans,
                            func: s.func.to_owned(),
                            // call_stack[0] is the program label (unchanging root);
                            // the actual function frames start at index 1.
                            call_depth: s.call_stack.len().saturating_sub(1).max(1),
                            cu_cost,
                            cu_cumulative: node_cu,
                            syscall: s.syscall,
                            src_loc,
                        });
                    },
                );

                tx_total_cu += node_cu;
                nodes.push(DebugNode {
                    program_label,
                    program_id: inv.program_id.clone(),
                    steps,
                });
            }

            if nodes.iter().all(|n| n.steps.is_empty()) {
                continue;
            }

            txs.push(DebugTx {
                test_name: test_name.clone(),
                tx_seq,
                total_cu: tx_total_cu,
                nodes,
            });
        }
    }

    // Stable order: test name, then tx seq. Matches how users read the test
    // output above the TUI.
    txs.sort_by(|a, b| {
        a.test_name
            .cmp(&b.test_name)
            .then_with(|| a.tx_seq.cmp(&b.tx_seq))
    });

    // Build the per-program static disasm map from whichever programs we
    // ended up resolving above. Doing this after the main loop lets us
    // reuse the already-loaded `ProgramCtx` instead of opening each ELF
    // a second time.
    let mut programs_disasm: BTreeMap<String, ProgramDisasm> = BTreeMap::new();
    for (pid, ctx) in &program_ctx {
        let mut d = build_static_disasm(ctx);
        d.has_dwarf = !ctx.source.is_empty();
        programs_disasm.insert(pid.clone(), d);
    }

    Ok(DebugSession {
        txs,
        src_roots,
        path_rewrites,
        cwd,
        programs: programs_disasm,
    })
}

struct GdbSidecars {
    regs: Vec<u8>,
    insns: Vec<u8>,
    cu_costs: Vec<u64>,
}

fn load_gdb_sidecars(
    inv: &InvocationFiles,
    regular_regs: &[u8],
    regular_insns: &[u8],
    regular_count: usize,
) -> Result<Option<GdbSidecars>> {
    let regs_path = inv.regs_path.with_extension("gdb.regs");
    let insns_path = inv.regs_path.with_extension("gdb.insns");
    let cu_path = inv.regs_path.with_extension("gdb.cu");

    if !regs_path.exists() && !insns_path.exists() && !cu_path.exists() {
        return Ok(None);
    }
    if !regs_path.exists() || !insns_path.exists() || !cu_path.exists() {
        bail!(
            "incomplete gdb sidecar set for {}; expected {}, {}, and {}",
            inv.regs_path.display(),
            regs_path.display(),
            insns_path.display(),
            cu_path.display(),
        );
    }

    let mut regs = fs::read(&regs_path).with_context(|| format!("read {}", regs_path.display()))?;
    let insns = fs::read(&insns_path).with_context(|| format!("read {}", insns_path.display()))?;
    let cu = fs::read(&cu_path).with_context(|| format!("read {}", cu_path.display()))?;
    let count = (regs.len() / REGS_ENTRY_SIZE).min(insns.len() / INSN_ENTRY_SIZE);
    if count != regular_count {
        bail!(
            "gdb sidecar count mismatch for {}: gdb has {count} steps, regular trace has \
             {regular_count}",
            inv.regs_path.display(),
        );
    }
    validate_and_normalize_gdb_regs(&mut regs, regular_regs, count)
        .with_context(|| format!("validate {}", regs_path.display()))?;
    if insns != regular_insns {
        bail!(
            "gdb sidecar instruction bytes differ from regular trace for {}",
            inv.regs_path.display(),
        );
    }
    if cu.len() / std::mem::size_of::<u64>() != count {
        bail!(
            "gdb sidecar CU count mismatch for {}: gdb has {} CU entries, trace has {count} steps",
            inv.regs_path.display(),
            cu.len() / std::mem::size_of::<u64>(),
        );
    }

    Ok(Some(GdbSidecars {
        regs,
        insns,
        cu_costs: cu_costs_from_remaining(&cu, count),
    }))
}

fn validate_and_normalize_gdb_regs(
    gdb_regs: &mut [u8],
    regular_regs: &[u8],
    count: usize,
) -> Result<()> {
    if gdb_regs.len() < count * REGS_ENTRY_SIZE || regular_regs.len() < count * REGS_ENTRY_SIZE {
        bail!("register trace is shorter than declared step count");
    }

    let mut text_addr = None::<u64>;
    for i in 0..count {
        let offset = i * REGS_ENTRY_SIZE;
        for reg in 0..12 {
            let start = offset + reg * 8;
            let gdb = u64::from_le_bytes(gdb_regs[start..start + 8].try_into().unwrap());
            let regular = u64::from_le_bytes(regular_regs[start..start + 8].try_into().unwrap());

            if reg == 11 {
                let expected_text_addr = gdb
                    .checked_sub(
                        regular
                            .checked_mul(INSN_ENTRY_SIZE as u64)
                            .ok_or_else(|| anyhow!("trace PC overflow at step {i}"))?,
                    )
                    .ok_or_else(|| anyhow!("gdb PC precedes trace PC at step {i}"))?;
                match text_addr {
                    Some(addr) if addr != expected_text_addr => bail!(
                        "gdb PC/text address mismatch at step {i}: expected text addr {addr:#x}, \
                         got {expected_text_addr:#x}",
                    ),
                    None => text_addr = Some(expected_text_addr),
                    _ => {}
                }
                gdb_regs[start..start + 8].copy_from_slice(&regular.to_le_bytes());
            } else if gdb != regular {
                bail!("gdb register r{reg} mismatch at step {i}: gdb={gdb:#x}, trace={regular:#x}",);
            }
        }
    }
    Ok(())
}

fn cu_costs_from_remaining(data: &[u8], count: usize) -> Vec<u64> {
    let mut costs = Vec::with_capacity(count);
    let mut prev = None::<u64>;
    for i in 0..count {
        let offset = i * std::mem::size_of::<u64>();
        let Some(bytes) = data.get(offset..offset + 8) else {
            costs.push(0);
            continue;
        };
        let remaining = u64::from_le_bytes(bytes.try_into().unwrap());
        costs.push(prev.map(|p| p.saturating_sub(remaining)).unwrap_or(0));
        prev = Some(remaining);
    }
    costs
}

/// Disassemble every PC in the program's text section in memory order,
/// pre-highlighting each line for the TUI hot path. Function-entry PCs
/// pick up a `func_label` so the static view renders symbol headers.
///
/// Cost is proportional to text-section size; for a typical 50KB SBF
/// `.text` (~6k insns) this runs in low single-digit ms.
fn build_static_disasm(ctx: &ProgramCtx) -> ProgramDisasm {
    let (_, text) = ctx.executable.get_text_bytes();
    // text_bytes is the raw text section; PCs are byte-offset / 8.
    let n = text.len() / solana_sbpf::ebpf::INSN_SIZE;
    let mut insns: Vec<StaticInsn> = Vec::with_capacity(n);
    let mut pc_to_idx: BTreeMap<u64, usize> = BTreeMap::new();

    let mut pc = 0;
    while pc < n {
        let mut raw = ebpf::get_insn_unchecked(text, pc);
        // `lddw` is the only SBPF instruction that occupies two slots:
        // pc N holds opcode + lower 32 bits of imm; pc N+1 holds opcode 0x0
        // + upper 32 bits in its imm field. Disassembling N+1 standalone
        // produces a phantom `unknown opcode=0x0` row. Merge the halves
        // and skip the second slot so the static view matches what the
        // VM actually executes.
        let mut step = 1;
        if raw.opc == ebpf::LD_DW_IMM && pc + 1 < n {
            ebpf::augment_lddw_unchecked(text, &mut raw);
            step = 2;
        }
        let disasm = ctx.analysis.disassemble_instruction(&raw, pc);
        let disasm_spans = highlight_asm(&disasm).spans;
        let func_label = ctx.symbols.get(&(pc as u64)).cloned();
        pc_to_idx.insert(pc as u64, insns.len());
        // Map the second-half PC to the same display row so any trace
        // step that lands on N+1 (shouldn't happen, but defensively) still
        // resolves to a valid entry.
        if step == 2 {
            pc_to_idx.insert((pc + 1) as u64, insns.len());
        }
        insns.push(StaticInsn {
            pc: pc as u64,
            disasm_spans,
            func_label,
        });
        pc += step;
    }

    // `has_dwarf` is fixed up by the caller (`build_session`) which has
    // the `ProgramCtx.source` resolver in scope. Default to `false` here
    // so a future caller that forgets the patch sees the conservative
    // "no DWARF" path instead of the wrong "PC unmapped" hint.
    ProgramDisasm {
        insns,
        pc_to_idx,
        has_dwarf: false,
    }
}

struct ProgramCtx {
    symbols: BTreeMap<u64, String>,
    syscalls: BTreeMap<u32, String>,
    /// Borrowed from a deliberately leaked `Box<Executable>`. The debugger
    /// holds ~1 of these per deployed program for the life of the process;
    /// leaking beats self-referential-struct gymnastics and the OS reclaims
    /// on exit. Not called from anywhere hot.
    analysis: Analysis<'static>,
    /// Same leaked executable backing `analysis`. Held here so the static
    /// disasm builder can pull the raw text bytes via `get_text_bytes`.
    executable: &'static solana_sbpf::elf::Executable<NoopCtx>,
    source: SourceResolver,
}

#[derive(Default)]
struct NoopCtx;
impl solana_sbpf::vm::ContextObject for NoopCtx {
    fn consume(&mut self, _amount: u64) {}
    fn get_remaining(&self) -> u64 {
        0
    }
}

/// No-op `BuiltinFunction<NoopCtx>` used to register syscall names in the
/// loader's function registry. Never called — we replay traces, never
/// execute — so the body is unreachable in practice.
fn syscall_stub(
    _vm: *mut solana_sbpf::vm::EbpfVm<NoopCtx>,
    _r1: u64,
    _r2: u64,
    _r3: u64,
    _r4: u64,
    _r5: u64,
) {
}

fn load_program_ctx<'a>(
    program_id: &str,
    programs: &BTreeMap<String, PathBuf>,
    manifest_dir: Option<&Path>,
    cache: &'a mut BTreeMap<String, ProgramCtx>,
) -> Option<&'a ProgramCtx> {
    if !cache.contains_key(program_id) {
        let ctx = build_program_ctx(program_id, programs, manifest_dir)?;
        cache.insert(program_id.to_owned(), ctx);
    }
    cache.get(program_id)
}

fn build_program_ctx(
    program_id: &str,
    programs: &BTreeMap<String, PathBuf>,
    manifest_dir: Option<&Path>,
) -> Option<ProgramCtx> {
    let elf_path = programs.get(program_id)?;
    let elf_bytes = fs::read(elf_path).ok()?;
    let (symbols, syscalls) = match load_function_map(elf_path, manifest_dir) {
        Ok(x) => x,
        Err(e) => {
            // Common cause: bench-style workspaces produce a raw cargo
            // SBF ELF in `target/sbpf-solana-solana/release/` that
            // solana-sbpf can't parse without `cargo build-sbf`'s
            // post-link step. Surface this clearly so the user knows
            // what to do instead of silently dropping the trace.
            eprintln!(
                "warning: can't parse {} ({e}). Run `cargo build-sbf -p <crate>` from the \
                 workspace root to produce a debugger-compatible `target/deploy/<name>.so`.",
                elf_path.display()
            );
            return None;
        }
    };

    // Build the loader with every known syscall registered so sbpf's
    // disassembler can name `CALL_IMM` syscalls instead of printing
    // `[invalid]`. The function pointer is a no-op stub — we never
    // actually invoke syscalls (we replay traces, not execute), so the
    // registry is consulted only for the (hash → name) lookup that
    // `disassembler::disassemble_instruction` does for CALL_IMM.
    let mut loader_inner =
        solana_sbpf::program::BuiltinProgram::new_loader(solana_sbpf::vm::Config {
            enable_symbol_and_section_labels: true,
            ..solana_sbpf::vm::Config::default()
        });
    for name in KNOWN_SYSCALLS {
        // Ignore registration errors — a duplicate hash would mean two
        // syscalls collide in `KNOWN_SYSCALLS`, which is a list bug, not
        // a per-program issue. Continuing yields a partial registry
        // (better than no names at all).
        let _ = loader_inner.register_function(name, syscall_stub);
    }
    let loader = Arc::new(loader_inner);
    let executable = solana_sbpf::elf::Executable::<NoopCtx>::from_elf(&elf_bytes, loader).ok()?;
    // Leak the executable so the `Analysis` can borrow it with a `'static`
    // lifetime. Each unique program gets leaked exactly once per debugger
    // session; the allocation is reclaimed when the process exits.
    let exec_ref: &'static solana_sbpf::elf::Executable<NoopCtx> = Box::leak(Box::new(executable));
    let analysis = Analysis::from_executable(exec_ref).ok()?;

    // DWARF lives in the unstripped build artifact, not `target/deploy/` —
    // `cargo-build-sbf` strips before copying. Fall back to the deployed
    // path if no unstripped sibling is available (third-party deploys etc.).
    let dwarf_path =
        find_unstripped_binary(elf_path, manifest_dir).unwrap_or_else(|| elf_path.to_path_buf());
    let source = SourceResolver::from_elf_path(&dwarf_path);

    Some(ProgramCtx {
        symbols,
        syscalls,
        analysis,
        executable: exec_ref,
        source,
    })
}

fn disassemble(analysis: &Analysis<'_>, insn_bytes: &[u8; 8], pc: usize) -> String {
    let insn = ebpf::Insn {
        ptr: pc,
        opc: insn_bytes[0],
        dst: insn_bytes[1] & 0x0f,
        src: (insn_bytes[1] & 0xf0) >> 4,
        off: i16::from_le_bytes([insn_bytes[2], insn_bytes[3]]),
        imm: i32::from_le_bytes([insn_bytes[4], insn_bytes[5], insn_bytes[6], insn_bytes[7]])
            as i64,
    };
    analysis.disassemble_instruction(&insn, pc)
}

fn program_label(program_id: &str, programs: &BTreeMap<String, PathBuf>) -> String {
    let short = short_pid(program_id);
    match programs.get(program_id) {
        Some(elf) => elf
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|n| format!("{n} ({short})"))
            .unwrap_or_else(|| format!("program {program_id}")),
        None => format!("[unresolved {short}]"),
    }
}

fn short_pid(pid: &str) -> String {
    if pid.len() <= 13 {
        pid.to_owned()
    } else {
        format!("{}…{}", &pid[..8], &pid[pid.len() - 4..])
    }
}
