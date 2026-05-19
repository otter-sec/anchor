//! `anchor coverage` — generates LCOV source-level coverage from SBF register
//! traces. Reuses the debugger's DWARF resolution to map executed PCs to
//! source lines.
//!
//! Trace collection uses litesvm's stock `register-tracing` feature (no forked
//! dependencies). Programs must be built with `CARGO_PROFILE_RELEASE_DEBUG=2`
//! to include DWARF in the unstripped `.so`.

use {
    crate::{
        debugger::source::SourceResolver,
        flamegraph::trace::{find_unstripped_binary, REGS_ENTRY_SIZE},
    },
    anyhow::{Context, Result},
    std::{
        collections::{BTreeMap, BTreeSet, HashMap},
        fs,
        io::Write,
        path::{Path, PathBuf},
    },
};

/// Generate an LCOV file from register trace data.
///
/// `trace_dir` — directory containing `.regs` files (litesvm `SBF_TRACE_DIR`).
/// `programs` — map of program_id (base58) → deployed `.so` path (from
/// `discover_programs`). The unstripped version is resolved automatically
/// via [`find_unstripped_binary`].
/// `manifest_dir` — workspace manifest dir; used to (1) locate unstripped
/// binaries and (2) resolve relative source paths emitted by DWARF (Solana's
/// cargo passes `-Zremap-cwd-prefix=` which strips `DW_AT_comp_dir`, so
/// paths come back as `lang-v2/src/cpi.rs` rather than absolute).
/// `output` — path to write the LCOV file.
///
/// Emitted entries are filtered to files that actually exist on disk. This
/// drops phantom paths from dependency crates (e.g. pinocchio's bare
/// `src/de/mod.rs`) that can't be resolved without per-crate context.
pub fn generate_lcov(
    trace_dir: &Path,
    programs: &BTreeMap<String, PathBuf>,
    manifest_dir: Option<&Path>,
    output: &Path,
) -> Result<()> {
    let pc_sets = collect_pcs_from_traces(trace_dir)?;
    if pc_sets.is_empty() {
        eprintln!("warning: no trace data found in {}", trace_dir.display());
        return Ok(());
    }

    eprintln!("found {} program(s) in traces", pc_sets.len());

    let mut line_hits: HashMap<PathBuf, BTreeMap<u32, u64>> = HashMap::new();

    for (program_id, pcs) in &pc_sets {
        let deployed = match programs.get(program_id) {
            Some(p) => p,
            None => {
                eprintln!("warning: no .so found for program {program_id}, skipping");
                continue;
            }
        };

        // DWARF lives in the unstripped sibling at
        // `<workspace_root>/target/sbpf-solana-solana/release/<name>.so`.
        // `find_unstripped_binary` walks up from `manifest_dir` to locate it
        // deterministically (no guessing, no SHA matching).
        let dwarf_path = find_unstripped_binary(deployed, manifest_dir)
            .unwrap_or_else(|| deployed.to_path_buf());

        let resolver = SourceResolver::from_elf_path(&dwarf_path);
        if resolver.is_empty() {
            eprintln!(
                "warning: no DWARF in {} — rebuild with CARGO_PROFILE_RELEASE_DEBUG=2",
                dwarf_path.display()
            );
            continue;
        }

        // Walk the full DWARF inlining chain per PC so `#[inline(always)]`
        // wrappers get direct coverage credit. `find_location` alone would
        // attribute the PC to whichever line the line program emits —
        // usually one frame, sometimes the outer callsite — leaving tiny
        // helpers like `Box<T>::load` and `AccountLoader::next*` at 0%
        // despite running on every transaction. Matches the behavior of
        // `llvm-cov show` over compile-time expansion regions.
        let mut resolved_count = 0u64;
        for &pc in pcs {
            let frames = resolver.resolve_frames(pc);
            if !frames.is_empty() {
                resolved_count += 1;
            }
            for loc in frames {
                if let Some(path) = resolve_source_path(&loc.file, manifest_dir) {
                    *line_hits
                        .entry(path)
                        .or_default()
                        .entry(loc.line)
                        .or_insert(0) += 1;
                }
            }
        }
        eprintln!(
            "  {} — {} unique PCs, {} resolved to source",
            dwarf_path.file_name().unwrap_or_default().to_string_lossy(),
            pcs.len(),
            resolved_count,
        );
    }

    // Write LCOV format.
    let mut out =
        fs::File::create(output).with_context(|| format!("create {}", output.display()))?;

    let mut sorted_files: Vec<_> = line_hits.into_iter().collect();
    sorted_files.sort_by(|a, b| a.0.cmp(&b.0));

    let total_files = sorted_files.len();
    let total_lines: usize = sorted_files.iter().map(|(_, l)| l.len()).sum();

    for (file, lines) in &sorted_files {
        writeln!(out, "SF:{}", file.display())?;
        for (&line, &hits) in lines {
            writeln!(out, "DA:{line},{hits}")?;
        }
        let lf = lines.len();
        let lh = lines.values().filter(|&&h| h > 0).count();
        writeln!(out, "LF:{lf}")?;
        writeln!(out, "LH:{lh}")?;
        writeln!(out, "end_of_record")?;
    }

    eprintln!("  {total_files} source files, {total_lines} lines covered");
    Ok(())
}

/// Resolve a DWARF-emitted source path to an absolute path that exists on
/// disk. Returns `None` if the file can't be found.
///
/// Solana's cargo passes `-Zremap-cwd-prefix=` which strips `DW_AT_comp_dir`,
/// so DWARF paths come back as either:
///   - absolute (e.g. `/Users/runner/...` for stdlib baked at CI-build time)
///   - relative to the invocation cwd (e.g. `lang-v2/src/cpi.rs` when `cargo
///     build-sbf` was invoked from the workspace root)
///   - bare relative `src/foo.rs` from dep crates — these can't be resolved
///     without per-crate context and are dropped.
fn resolve_source_path(file: &Path, workspace_root: Option<&Path>) -> Option<PathBuf> {
    if file.is_absolute() {
        return file.exists().then(|| file.to_path_buf());
    }
    let root = workspace_root?;
    let candidate = root.join(file);
    candidate.exists().then_some(candidate)
}

/// Walk trace directory recursively, collecting all unique PCs (reg[11])
/// per program_id from `.regs` files.
///
/// Handles both trace-dir layouts:
///   - flat `<dir>/<hash>.regs` (litesvm's `SBF_TRACE_DIR`)
///   - nested `<dir>/<test_name>/<inv>__tx<N>.regs` (anchor-v2-testing's
///     `ANCHOR_PROFILE_DIR`, used by `anchor debugger`)
fn collect_pcs_from_traces(trace_dir: &Path) -> Result<BTreeMap<String, BTreeSet<u64>>> {
    let mut result: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();

    if !trace_dir.exists() {
        return Ok(result);
    }

    visit_dir(trace_dir, &mut result)?;
    Ok(result)
}

fn visit_dir(dir: &Path, result: &mut BTreeMap<String, BTreeSet<u64>>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            visit_dir(&path, result)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) != Some("regs") {
            continue;
        }

        let pid_path = path.with_extension("program_id");
        let program_id = match fs::read_to_string(&pid_path) {
            Ok(s) => s.trim().to_string(),
            Err(_) => continue,
        };

        let data = fs::read(&path)?;
        if data.len() % REGS_ENTRY_SIZE != 0 {
            eprintln!(
                "warning: {} has unexpected size (not multiple of {})",
                path.display(),
                REGS_ENTRY_SIZE
            );
            continue;
        }

        let pcs = result.entry(program_id).or_default();
        let num_steps = data.len() / REGS_ENTRY_SIZE;
        for i in 0..num_steps {
            let offset = i * REGS_ENTRY_SIZE + 11 * 8;
            let pc = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
            pcs.insert(pc);
        }
    }
    Ok(())
}
