use {
    anyhow::{anyhow, Context, Result},
    object::{Object, ObjectSection, ObjectSymbol, SymbolKind},
    rustc_demangle::demangle,
    solana_compute_budget::compute_budget::ComputeBudget,
    solana_sbpf::{
        ebpf,
        elf::Executable,
        program::BuiltinProgram,
        static_analysis::Analysis,
        vm::{Config, ContextObject},
    },
    std::{collections::BTreeMap, fs, path::Path, sync::Arc},
};

/// Returns the base compute-unit cost agave charges for `syscall_name`.
///
/// Pulled directly from `ComputeBudget` constants so the numbers match
/// what the runtime would actually meter (minus variable parts we can't
/// infer from registers — big-buffer hash/log costs in particular).
///
/// Unknown syscalls fall back to `syscall_base_cost`, which is what
/// agave itself uses as the floor for "any syscall at all" work.
fn syscall_cost(budget: &ComputeBudget, syscall_name: &str) -> u64 {
    match syscall_name {
        "sol_log_64_" => budget.log_64_units,
        "sol_log_pubkey" => budget.log_pubkey_units,
        "sol_sha256" | "sol_keccak256" | "sol_blake3" => budget.sha256_base_cost,
        "sol_secp256k1_recover" => budget.secp256k1_recover_cost,
        "sol_invoke_signed_c" | "sol_invoke_signed_rust" => budget.invoke_units,
        "sol_create_program_address" | "sol_try_find_program_address" => {
            budget.create_program_address_units
        }
        "sol_memcpy_" | "sol_memmove_" | "sol_memset_" | "sol_memcmp_" => budget.mem_op_base_cost,
        "sol_get_clock_sysvar"
        | "sol_get_epoch_schedule_sysvar"
        | "sol_get_fees_sysvar"
        | "sol_get_rent_sysvar"
        | "sol_get_last_restart_slot"
        | "sol_get_epoch_rewards_sysvar"
        | "sol_get_sysvar" => budget.sysvar_base_cost,
        "sol_curve_validate_point" => budget.curve25519_edwards_validate_point_cost,
        "sol_curve_group_op" => budget.curve25519_edwards_add_cost,
        "sol_big_mod_exp" => budget.big_modular_exponentiation_base_cost,
        "sol_remaining_compute_units" => budget.get_remaining_compute_units_cost,
        "sol_alt_bn128_compression" => budget.alt_bn128_g1_compress,
        "sol_alt_bn128_group_op" => budget.alt_bn128_addition_cost,
        "sol_poseidon" => budget.poseidon_cost_coefficient_c,
        // Includes sol_log_, sol_log_data, sol_log_compute_units_, abort,
        // sol_panic_, sol_set_return_data, sol_get_return_data,
        // sol_get_stack_height, sol_get_epoch_stake,
        // sol_get_processed_sibling_instruction, and anything agave added
        // that we haven't mapped yet.
        _ => budget.syscall_base_cost,
    }
}

pub struct FlamegraphReport {
    pub program_name: String,
    pub total_cu: u64,
    pub stacks: BTreeMap<Vec<String>, u64>,
}

/// Size in bytes of one register trace entry: 12 x u64 = 96 bytes.
pub const REGS_ENTRY_SIZE: usize = 12 * std::mem::size_of::<u64>();

/// Size in bytes of one raw SBPF instruction: 8 bytes.
pub const INSN_ENTRY_SIZE: usize = 8;

/// One step yielded by [`stream_trace`].
///
/// Borrowed from the caller's maintained call stack so consumers that only
/// care about a snapshot can clone into owned state on demand.
pub struct StreamStep<'a> {
    pub pc: u64,
    pub regs: [u64; 12],
    pub insn: [u8; 8],
    /// Call stack from program-root to current frame. Length >= 1.
    pub call_stack: &'a [String],
    /// Resolved function display name for this PC (same as `call_stack.last()`
    /// with the `@ {entry_pc:#x}` suffix stripped).
    pub func: &'a str,
    /// `Some(name)` if this step is a syscall leaf (SBPFv1 `CALL_IMM` where
    /// the next traced PC == pc+1); `None` otherwise.
    pub syscall: Option<String>,
    /// CU cost attributed to this step: 1 for a plain instruction, or the
    /// syscall base cost (from `ComputeBudget`) for a syscall leaf.
    pub cu_cost: u64,
}

/// Provides the minimal VM context needed to parse and inspect an executable.
#[derive(Default)]
struct NoopContext;

impl ContextObject for NoopContext {
    fn consume(&mut self, _amount: u64) {}
    fn get_remaining(&self) -> u64 {
        0
    }
}

/// Streams a trace (regs + insns pair), invoking `visit` for every step with
/// the up-to-date call stack, resolved function, CU cost, and (for syscall
/// leaves) the syscall name.
///
/// The algorithm is PC-driven rather than opcode-driven: for every traced
/// instruction we look up which function the PC falls into and resync the
/// maintained call stack against it. If the resolved function is already in
/// the stack we pop down to it (we missed one or more returns); if it's not
/// in the stack we push it (we missed a call, e.g. via a tail call or direct
/// branch). This is more robust than dispatching off `EXIT`/`RETURN` opcodes
/// because direct jumps between functions never produce a matching call/ret
/// pair.
///
/// The one case we still special-case is SBPFv1's `CALL_IMM` → syscall: a
/// syscall does not change the persistent call stack (the caller resumes at
/// the next instruction), but we still want to attribute its single traced
/// CU to a `[syscall] {name}` leaf frame for display.
pub fn stream_trace(
    regs_data: &[u8],
    insns_data: &[u8],
    count: usize,
    symbol_map: &BTreeMap<u64, String>,
    syscall_names: &BTreeMap<u32, String>,
    program_name: &str,
    budget: &ComputeBudget,
    mut visit: impl FnMut(StreamStep<'_>),
) {
    // Track the call stack by maintaining a stack of function display names.
    // The root frame is the program name and is never popped. Every other
    // frame is formatted as `{function_name} @ {entry_pc:#x}` so the SVG
    // shows both the resolved symbol and its SBPF entry point.
    let mut call_stack: Vec<String> = vec![program_name.to_owned()];

    for i in 0..count {
        let regs = read_regs(regs_data, i);
        let insn = read_insn(insns_data, i);
        let pc = regs[11];

        // Resolve the function containing this PC along with its entry PC,
        // and build the display frame name we'd use if we had to push it.
        let (current_name, current_entry_pc) = lookup_function_with_pc(symbol_map, pc);
        let current_frame = format!("{current_name} @ {current_entry_pc:#x}");

        // PC-driven call-stack resync. If the top of the stack no longer
        // matches the current frame, either we missed one or more returns
        // (pop down to `current_frame` if it is anywhere in the stack) or we
        // missed a call (push `current_frame`).
        if call_stack.last().map(String::as_str) != Some(current_frame.as_str()) {
            if let Some(depth) = call_stack.iter().rposition(|f| f == &current_frame) {
                call_stack.truncate(depth + 1);
            } else {
                call_stack.push(current_frame);
            }
        }

        // SBPFv1 CALL_IMM is overloaded: it's either an internal call (PC
        // jumps to the callee on the next trace entry) or an external
        // syscall (PC advances by 1). We only need to intercept the syscall
        // case — internal calls are handled by the resync above on the next
        // iteration, which will see the callee's PC and push it.
        if insn[0] == ebpf::CALL_IMM {
            let imm = u32::from_le_bytes(insn[4..8].try_into().unwrap());
            let is_syscall = if i + 1 < count {
                let next_regs = read_regs(regs_data, i + 1);
                let next_pc = next_regs[11];
                // Syscall: PC advances by 1 (next sequential instruction).
                // Internal call: PC jumps to a different location.
                next_pc == pc + 1
            } else {
                // Last traced instruction — treat as internal call so the
                // call site is attributed to the current frame rather than
                // an unresolvable syscall stub.
                false
            };

            if is_syscall {
                let syscall_name = syscall_names
                    .get(&imm)
                    .cloned()
                    .unwrap_or_else(|| format!("syscall_{imm:#x}"));
                let cost = syscall_cost(budget, &syscall_name);
                visit(StreamStep {
                    pc,
                    regs,
                    insn,
                    call_stack: &call_stack,
                    func: &current_name,
                    syscall: Some(syscall_name),
                    cu_cost: cost,
                });
                continue;
            }
        }

        // Default attribution: 1 CU to the current call-stack top. EXIT /
        // RETURN opcodes fall into this arm too — their pop is handled
        // implicitly by the next iteration's PC-driven resync.
        visit(StreamStep {
            pc,
            regs,
            insn,
            call_stack: &call_stack,
            func: &current_name,
            syscall: None,
            cu_cost: 1,
        });
    }
}

/// Folds a trace into SVG stacks + total CU by consuming [`stream_trace`].
fn process_trace(
    regs_data: &[u8],
    insns_data: &[u8],
    count: usize,
    symbol_map: &BTreeMap<u64, String>,
    syscall_names: &BTreeMap<u32, String>,
    program_name: &str,
    budget: &ComputeBudget,
) -> (BTreeMap<Vec<String>, u64>, u64) {
    let mut folded_stacks: BTreeMap<Vec<String>, u64> = BTreeMap::new();
    let mut total_cu: u64 = 0;

    stream_trace(
        regs_data,
        insns_data,
        count,
        symbol_map,
        syscall_names,
        program_name,
        budget,
        |step| {
            total_cu += step.cu_cost;
            if let Some(syscall) = &step.syscall {
                let mut stack: Vec<String> = step.call_stack.to_vec();
                stack.push(format!("[syscall] {syscall}"));
                *folded_stacks.entry(stack).or_default() += step.cu_cost;
            } else {
                *folded_stacks.entry(step.call_stack.to_vec()).or_default() += step.cu_cost;
            }
        },
    );

    (folded_stacks, total_cu)
}

/// Reads the i-th register entry (12 x u64) from raw bytes.
pub fn read_regs(data: &[u8], i: usize) -> [u64; 12] {
    let offset = i * REGS_ENTRY_SIZE;
    let mut regs = [0u64; 12];
    for r in 0..12 {
        let start = offset + r * 8;
        let bytes: [u8; 8] = data[start..start + 8].try_into().unwrap();
        regs[r] = u64::from_le_bytes(bytes);
    }
    regs
}

/// Reads the i-th instruction entry (8 bytes) from raw bytes.
pub fn read_insn(data: &[u8], i: usize) -> [u8; 8] {
    let offset = i * INSN_ENTRY_SIZE;
    data[offset..offset + 8].try_into().unwrap()
}

/// Looks up the function name AND entry PC for a given PC using the symbol
/// map, by walking to the nearest lower-or-equal symbol entry. Returns
/// `("unknown_{pc:#x}", pc)` as a fallback when the map has no covering
/// entry.
pub fn lookup_function_with_pc(symbol_map: &BTreeMap<u64, String>, pc: u64) -> (String, u64) {
    symbol_map
        .range(..=pc)
        .next_back()
        .map(|(entry_pc, name)| (name.clone(), *entry_pc))
        .unwrap_or_else(|| (format!("unknown_{pc:#x}"), pc))
}

/// One invocation's trace files + the program it ran.
pub struct InvocationFiles {
    pub inv_seq: u32,
    pub tx_seq: u32,
    pub program_id: String,
    pub regs_path: std::path::PathBuf,
    pub insns_path: std::path::PathBuf,
}

/// Parses filenames like `0001__tx2.regs` (written by
/// `anchor-v2-testing`'s `TestNameCallback`) and groups everything
/// under a test directory into a stable, tx-ordered list of
/// invocations. Ignores files that don't match the expected layout or
/// are missing a sibling.
pub fn discover_invocations(trace_dir: &Path) -> Result<Vec<InvocationFiles>> {
    let mut found = Vec::new();
    if !trace_dir.exists() {
        return Ok(found);
    }

    let entries = fs::read_dir(trace_dir)
        .with_context(|| format!("Failed to read trace directory {}", trace_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("regs") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        // Expect `NNNN__txN`.
        let Some((inv_part, tx_part)) = stem.split_once("__") else {
            continue;
        };
        let Some(tx_digits) = tx_part.strip_prefix("tx") else {
            continue;
        };
        let Ok(inv_seq) = inv_part.parse::<u32>() else {
            continue;
        };
        let Ok(tx_seq) = tx_digits.parse::<u32>() else {
            continue;
        };

        let insns_path = path.with_extension("insns");
        let pid_path = path.with_extension("program_id");
        if !insns_path.exists() || !pid_path.exists() {
            continue;
        }
        let program_id = match fs::read_to_string(&pid_path) {
            Ok(s) => s.trim().to_owned(),
            Err(_) => continue,
        };

        found.push(InvocationFiles {
            inv_seq,
            tx_seq,
            program_id,
            regs_path: path,
            insns_path,
        });
    }

    // Deterministic: ascending by (tx, inv).
    found.sort_by_key(|f| (f.tx_seq, f.inv_seq));
    Ok(found)
}

/// Build one [`FlamegraphReport`] per outer transaction in the test.
///
/// Each tx's report folds together every invocation (top-level +
/// CPIs) within that tx, symbolicating each invocation against its
/// own program's ELF. Invocations whose `program_id` has no entry in
/// `programs` contribute frames labeled `[unresolved <pid>]` so CUs
/// aren't silently dropped.
///
/// Returns reports keyed by `tx_seq` (1-indexed, matching the
/// `TestNameCallback`'s before-invocation counter). Empty map if the
/// directory has no parseable traces.
pub fn build_tx_reports(
    test_name: &str,
    test_dir: &Path,
    programs: &std::collections::BTreeMap<String, std::path::PathBuf>,
    manifest_dir: Option<&Path>,
) -> Result<std::collections::BTreeMap<u32, FlamegraphReport>> {
    let invocations = discover_invocations(test_dir)?;
    if invocations.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }

    // Cache symbol maps per program — loading an ELF is expensive.
    let mut symbol_cache: std::collections::BTreeMap<
        String,
        (BTreeMap<u64, String>, BTreeMap<u32, String>),
    > = std::collections::BTreeMap::new();
    for (pid, elf) in programs {
        if let Ok(maps) = load_function_map(elf, manifest_dir) {
            symbol_cache.insert(pid.clone(), maps);
        }
    }

    // Fallback (empty) maps for invocations whose program we can't
    // resolve. We still want their CU accounted for, just as flat
    // `[unresolved]` stacks.
    let empty_symbols: BTreeMap<u64, String> = BTreeMap::new();
    let empty_syscalls: BTreeMap<u32, String> = BTreeMap::new();

    let mut reports: std::collections::BTreeMap<u32, (BTreeMap<Vec<String>, u64>, u64)> =
        std::collections::BTreeMap::new();
    let budget = ComputeBudget::new_with_defaults(false, false);

    for inv in &invocations {
        let regs = fs::read(&inv.regs_path)
            .with_context(|| format!("read {}", inv.regs_path.display()))?;
        let insns = fs::read(&inv.insns_path)
            .with_context(|| format!("read {}", inv.insns_path.display()))?;
        let count = (regs.len() / REGS_ENTRY_SIZE).min(insns.len() / INSN_ENTRY_SIZE);
        if count == 0 {
            continue;
        }

        let (symbols, syscalls) = match symbol_cache.get(&inv.program_id) {
            Some((s, c)) => (s, c),
            None => (&empty_symbols, &empty_syscalls),
        };

        // Label the program frame by short program id (first 8 +
        // last 4 chars) so it's identifiable but not taking half the
        // SVG width. Resolved programs use a nicer name when the ELF
        // filename is known.
        let program_label = match programs.get(&inv.program_id) {
            Some(elf_path) => {
                let short_pid = short_pid(&inv.program_id);
                elf_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|n| format!("[program {n} ({short_pid})]"))
                    .unwrap_or_else(|| format!("[program {}]", inv.program_id))
            }
            None => format!("[unresolved {}]", short_pid(&inv.program_id)),
        };

        let (stacks, cu) = process_trace(
            &regs,
            &insns,
            count,
            symbols,
            syscalls,
            &program_label,
            &budget,
        );
        let entry = reports
            .entry(inv.tx_seq)
            .or_insert_with(|| (BTreeMap::new(), 0));
        for (stack, cost) in stacks {
            *entry.0.entry(stack).or_default() += cost;
        }
        entry.1 += cu;
    }

    Ok(reports
        .into_iter()
        .filter(|(_, (_, cu))| *cu > 0)
        .map(|(tx_seq, (stacks, cu))| {
            (
                tx_seq,
                FlamegraphReport {
                    program_name: format!("{test_name} · tx{tx_seq}"),
                    total_cu: cu,
                    stacks,
                },
            )
        })
        .collect())
}

fn short_pid(pid: &str) -> String {
    if pid.len() <= 13 {
        pid.to_owned()
    } else {
        format!("{}…{}", &pid[..8], &pid[pid.len() - 4..])
    }
}

/// Loads function symbols from an ELF file using the SBPF loader.
///
/// This parses the ELF with `solana_sbpf` to extract the function registry and
/// analysis labels, which contain all internal function entry points with their
/// demangled names. Additionally loads symbols from the ELF's object-level symbol
/// table (dynamic symbols) as a fallback.
///
/// If the deployed binary is stripped (common for `cargo-build-sbf`), we also
/// try loading symbols from the unstripped build artifact in the
/// `target/sbpf-solana-solana/release/` directory.
pub fn load_function_map(
    elf_path: &Path,
    manifest_dir: Option<&Path>,
) -> Result<(BTreeMap<u64, String>, BTreeMap<u32, String>)> {
    let elf_bytes =
        fs::read(elf_path).with_context(|| format!("Failed to read ELF {}", elf_path.display()))?;

    // Parse through SBPF to get function registry labels.
    let loader = Arc::new(BuiltinProgram::new_loader(Config {
        enable_symbol_and_section_labels: true,
        ..Config::default()
    }));

    let executable = Executable::<NoopContext>::from_elf(&elf_bytes, loader)
        .map_err(|err| anyhow!("Failed to parse SBPF ELF {}: {err}", elf_path.display()))?;

    let analysis = Analysis::from_executable(&executable)
        .map_err(|err| anyhow!("Failed to analyze SBPF executable: {err}"))?;

    let mut symbols: BTreeMap<u64, String> = BTreeMap::new();

    // Primary source: SBPF analysis function labels (internal function registry).
    for (pc, (_key, name)) in analysis.functions.iter() {
        let normalized = normalize_symbol(name, *pc);
        symbols.entry(*pc as u64).or_insert(normalized);
    }

    // Secondary source: symbols from the deployed ELF (often stripped).
    if let Ok(extra) = load_elf_symbols(&elf_bytes) {
        for (pc, name) in extra {
            symbols.entry(pc as u64).or_insert(name);
        }
    }

    // Tertiary source: the unstripped pre-deploy binary in the build directory.
    // cargo-build-sbf strips the binary before copying to target/deploy/, but
    // the unstripped version remains in target/sbpf-solana-solana/release/.
    if let Some(unstripped_path) = find_unstripped_binary(elf_path, manifest_dir) {
        if let Ok(unstripped_bytes) = fs::read(&unstripped_path) {
            if let Ok(extra) = load_elf_symbols(&unstripped_bytes) {
                for (pc, name) in extra {
                    // Only overwrite generic "function_N" labels.
                    let entry = symbols.entry(pc as u64);
                    match entry {
                        std::collections::btree_map::Entry::Vacant(v) => {
                            v.insert(name);
                        }
                        std::collections::btree_map::Entry::Occupied(mut o) => {
                            if o.get().starts_with("function_") {
                                o.insert(name);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok((symbols, syscall_hash_map()))
}

/// Known syscall names registered by `agave`/`solana-program-runtime`. Used
/// both for symbol resolution in trace processing and to seed sbpf's loader
/// registry so its disassembler can name `CALL_IMM` syscalls instead of
/// printing `[invalid]`.
pub const KNOWN_SYSCALLS: &[&str] = &[
    // Panics / aborts.
    "abort",
    // Logging.
    "sol_log_",
    "sol_log_64_",
    "sol_log_compute_units_",
    "sol_log_data",
    "sol_log_pubkey",
    // Hashing + crypto.
    "sol_sha256",
    "sol_keccak256",
    "sol_blake3",
    "sol_poseidon",
    "sol_secp256k1_recover",
    "sol_curve_validate_point",
    "sol_curve_group_op",
    "sol_curve_multiscalar_mul",
    "sol_curve_pairing_map",
    "sol_alt_bn128_group_op",
    "sol_alt_bn128_compression",
    "sol_big_mod_exp",
    // CPIs.
    "sol_invoke_signed_c",
    "sol_invoke_signed_rust",
    // Return data.
    "sol_set_return_data",
    "sol_get_return_data",
    // PDAs.
    "sol_create_program_address",
    "sol_try_find_program_address",
    // Memory ops.
    "sol_memcpy_",
    "sol_memmove_",
    "sol_memset_",
    "sol_memcmp_",
    // Sysvars (individual fast paths + the generic dispatcher).
    "sol_get_clock_sysvar",
    "sol_get_epoch_schedule_sysvar",
    "sol_get_fees_sysvar",
    "sol_get_rent_sysvar",
    "sol_get_last_restart_slot",
    "sol_get_epoch_rewards_sysvar",
    "sol_get_sysvar",
    // Misc.
    "sol_panic_",
    "sol_get_processed_sibling_instruction",
    "sol_get_stack_height",
    "sol_remaining_compute_units",
    "sol_get_epoch_stake",
];

/// Build the syscall hash → name map for trace processing. Hashes are
/// computed on the fly via `solana_sbpf::ebpf::hash_symbol_name` so we
/// never have to keep a table of hex magic numbers in sync.
///
/// When a new syscall lands in agave-syscalls, add its name to
/// [`KNOWN_SYSCALLS`] and it'll automatically get symbolicated.
fn syscall_hash_map() -> BTreeMap<u32, String> {
    KNOWN_SYSCALLS
        .iter()
        .map(|name| {
            (
                solana_sbpf::ebpf::hash_symbol_name(name.as_bytes()),
                (*name).to_owned(),
            )
        })
        .collect()
}

/// Tries to locate the unstripped SBF binary for a deployed program.
///
/// `cargo-build-sbf --sbf-out-dir <dir>` copies a **stripped** .so into
/// `<dir>/`, but the unstripped build artifact remains in the cargo target
/// tree of whichever workspace actually compiled the program. In the bench
/// setup that can be any of:
///
///   - `<bench>/target/sbpf-solana-solana/release/<name>.so` — for programs
///     that are bench-workspace members (anchor v1 / v2).
///   - `<bench>/programs/<family>/<variant>/target/sbpf-solana-solana/release/<name>.so`
///     — for programs with their own `[workspace]` (pinocchio / steel / quasar).
///   - `<repo>/target/sbpf-solana-solana/release/<name>.so` — historical
///     location when building from the repo root.
///
/// Rather than enumerate every path combinatorially, we walk upward from the
/// deployed .so until we hit a directory that contains a `bench/` or
/// `programs/` sibling (the repo root-ish), then do a bounded recursive
/// search under it for a file matching `name` inside any
/// `sbpf-solana-solana/release` directory. Returns the first non-stripped
/// match, or `None` if nothing is found.
pub fn find_unstripped_binary(
    deployed_path: &Path,
    manifest_dir: Option<&Path>,
) -> Option<std::path::PathBuf> {
    let file_name = deployed_path.file_name()?.to_str()?.to_owned();

    // Preferred path: walk up from the manifest dir to find the nearest
    // containing `[workspace]` Cargo.toml (the workspace root that actually
    // built the program). For an isolated `[workspace]` program the root is
    // the manifest dir itself; for a bench-workspace member it's several
    // levels up (e.g. `bench/programs/helloworld/anchor-v1` → `bench/`).
    // Cargo always places build artifacts under `<workspace_root>/target/`,
    // so this gives us a precise lookup with no ambiguity — the same
    // program rebuilt at a different lib name couldn't leak a stale binary
    // because the workspace root is deterministic from the manifest path.
    if let Some(manifest) = manifest_dir {
        if let Some(root) = find_workspace_root(manifest) {
            let direct = root
                .join("target")
                .join("sbpf-solana-solana")
                .join("release")
                .join(&file_name);
            if direct.exists() {
                return Some(direct);
            }
            let deps = root
                .join("target")
                .join("sbpf-solana-solana")
                .join("release")
                .join("deps")
                .join(&file_name);
            if deps.exists() {
                return Some(deps);
            }
        }
    }

    // Fallback: walk up the deployed path to a plausible repo root and
    // recursively search for the file under any `sbpf-solana-solana/release`
    // directory. This handles historical layouts and cases where no
    // manifest_dir was supplied.
    let mut root = deployed_path.parent()?;
    loop {
        let parent = root.parent()?;
        let has_bench = parent.join("bench").is_dir();
        let has_target = parent.join("target").is_dir();
        if has_bench || has_target {
            root = parent;
            break;
        }
        root = parent;
    }

    search_for_unstripped(root, &file_name, 0)
}

/// Walks up from `manifest_dir` looking for a `Cargo.toml` that declares a
/// `[workspace]` table. Returns the directory of the first such manifest, or
/// `manifest_dir` itself if none is found (assumes single-crate repo).
fn find_workspace_root(manifest_dir: &Path) -> Option<std::path::PathBuf> {
    let mut current: std::path::PathBuf = manifest_dir.to_path_buf();
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = fs::read_to_string(&cargo_toml) {
                // Crude but good enough: the marker `[workspace]` at the
                // start of a line or after a newline means this Cargo.toml
                // defines a workspace.
                if contents.contains("\n[workspace]") || contents.starts_with("[workspace]") {
                    return Some(current);
                }
            }
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Recursively searches `dir` for `file_name` inside any
/// `sbpf-solana-solana/release` subdirectory. Depth-limited to avoid
/// pathological descent into dependencies. Returns the first match that is
/// not stripped (larger than the matching deployed .so would be — we can't
/// cheaply check strip state, but filtering by "inside release" usually
/// works since cargo's own target tree always keeps symbols).
fn search_for_unstripped(dir: &Path, file_name: &str, depth: usize) -> Option<std::path::PathBuf> {
    // Hard cap on recursion: repo layouts never nest workspaces more than
    // ~4 deep (e.g. `<repo>/bench/programs/<family>/<variant>/target/...`).
    // 6 is generous and cheap.
    if depth > 6 {
        return None;
    }

    // Quick check: does this directory already contain the file we want at
    // the expected `sbpf-solana-solana/release/<file>` path?
    let direct = dir
        .join("target")
        .join("sbpf-solana-solana")
        .join("release")
        .join(file_name);
    if direct.exists() {
        return Some(direct);
    }
    let deps = dir
        .join("target")
        .join("sbpf-solana-solana")
        .join("release")
        .join("deps")
        .join(file_name);
    if deps.exists() {
        return Some(deps);
    }

    // Recurse into subdirectories — but skip well-known noisy subtrees.
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Skip hidden dirs, target/deploy (stripped), node_modules, .git.
        if name.starts_with('.')
            || name == "node_modules"
            || name == "deploy"
            || name == "deploy-debug"
        {
            continue;
        }
        if let Some(found) = search_for_unstripped(&path, file_name, depth + 1) {
            return Some(found);
        }
    }

    None
}

/// Loads text symbols from an ELF file's symbol tables and maps them to SBPF PCs.
fn load_elf_symbols(elf_bytes: &[u8]) -> Result<BTreeMap<usize, String>> {
    let file = object::File::parse(elf_bytes)
        .map_err(|err| anyhow!("Failed to parse ELF for symbols: {err}"))?;

    let text_section = file
        .sections()
        .find(|s| s.name().ok() == Some(".text"))
        .or_else(|| {
            file.sections()
                .find(|s| s.kind() == object::SectionKind::Text)
        })
        .ok_or_else(|| anyhow!("No .text section"))?;

    let text_address = text_section.address();
    let text_end = text_address.saturating_add(text_section.size());

    let mut symbols = BTreeMap::new();

    for symbol in file.symbols().chain(file.dynamic_symbols()) {
        if symbol.kind() != SymbolKind::Text || symbol.address() == 0 {
            continue;
        }
        let Ok(name) = symbol.name() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        let address = symbol.address();
        if address < text_address || address >= text_end {
            continue;
        }

        let relative = address - text_address;
        if relative % ebpf::INSN_SIZE as u64 != 0 {
            continue;
        }

        let pc = (relative / ebpf::INSN_SIZE as u64) as usize;
        symbols
            .entry(pc)
            .or_insert_with(|| normalize_symbol(name, pc));
    }

    Ok(symbols)
}

/// Demangles and normalizes a raw symbol for flamegraph display.
fn normalize_symbol(name: &str, pc: usize) -> String {
    let trimmed = name.trim_matches(char::from(0));
    let normalized = if trimmed.is_empty() {
        format!("function_{pc}")
    } else {
        demangle(trimmed).to_string()
    };

    let cleaned = strip_rust_hash_suffix(&normalized)
        .replace(';', ":")
        .replace('\n', " ");

    shorten_qualified_name(&cleaned)
}

/// Removes the trailing rustc symbol hash suffix when present.
fn strip_rust_hash_suffix(symbol: &str) -> &str {
    let Some((prefix, suffix)) = symbol.rsplit_once("::h") else {
        return symbol;
    };

    if suffix.len() == 16 && suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
        prefix
    } else {
        symbol
    }
}

/// Shortens a fully qualified Rust name to at most the last 3 segments.
fn shorten_qualified_name(name: &str) -> String {
    // Don't shorten names that contain generic parameters or closures.
    if name.contains('<') || name.contains('{') || name.matches("::").count() <= 2 {
        return name.to_owned();
    }

    let parts: Vec<&str> = name.split("::").collect();
    if parts.len() <= 3 {
        return name.to_owned();
    }

    parts[parts.len() - 3..].join("::")
}
