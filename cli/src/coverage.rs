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
    anyhow::{bail, Context, Result},
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

/// Filter host-side LCOV before combining it with SBF trace coverage.
///
/// Host coverage links SBF-only program crates but does not execute their
/// handlers natively, so it can report `DA:N,0` for source that SBF did execute.
/// Keep genuinely uncovered executable zeroes, but drop host zeroes that are
/// either contradicted by an SBF hit on the same line or known non-executable
/// Rust source lines.
pub fn filter_host_lcov(sbf_lcov: &Path, host_lcov: &Path, output: &Path) -> Result<()> {
    let sbf_records = parse_lcov(sbf_lcov)?;
    let host_records = parse_lcov(host_lcov)?;
    let sbf_hit_lines = sbf_records
        .iter()
        .map(|record| {
            (
                record.path.clone(),
                record
                    .da_counts
                    .iter()
                    .filter_map(|(line, count)| (*count > 0).then_some((*line, *count)))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .filter(|(_, lines)| !lines.is_empty())
        .collect::<BTreeMap<_, _>>();
    let source_suppression = build_source_suppression(&sbf_records)?;

    let mut out = String::new();
    let mut exact_suppressed = 0usize;
    let mut source_suppressed = 0usize;
    let mut function_hits_inferred = 0usize;

    for record in &host_records {
        let source_lines = source_suppression.get(&record.path);
        let sbf_lines = sbf_hit_lines.get(&record.path);
        let function_starts = parse_function_starts(record)?;
        let function_ranges = parse_function_ranges(record, &function_starts)?;
        let (filtered_lines, inferred) =
            infer_sbf_function_hits(record, sbf_lines, &function_starts, &function_ranges)
                .with_context(|| {
                    format!("filter function coverage for {}", record.path.display())
                })?;
        function_hits_inferred += inferred;

        for line in &filtered_lines {
            if let Some((line_no, count)) = parse_da_line(line)? {
                if count == 0 && sbf_lines.is_some_and(|lines| lines.contains_key(&line_no)) {
                    exact_suppressed += 1;
                    continue;
                }
                if count == 0
                    && !sbf_lines.is_some_and(|lines| lines.contains_key(&line_no))
                    && source_lines.is_some_and(|lines| lines.contains(&line_no))
                {
                    source_suppressed += 1;
                    continue;
                }
            }
            out.push_str(line);
            out.push('\n');
        }
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, out).with_context(|| format!("write {}", output.display()))?;
    eprintln!(
        "filtered host zero-hit DA lines: {exact_suppressed} exact SBF line hits, \
         {source_suppressed} non-executable Rust source lines, \
         {function_hits_inferred} SBF-backed function hits"
    );

    Ok(())
}

#[derive(Debug)]
struct LcovRecord {
    path: PathBuf,
    lines: Vec<String>,
    da_counts: BTreeMap<u32, i64>,
}

fn parse_lcov(path: &Path) -> Result<Vec<LcovRecord>> {
    let content = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut records = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_lines = Vec::new();
    let mut current_da = BTreeMap::new();

    for line in content.lines() {
        if let Some(sf) = line.strip_prefix("SF:") {
            if current_path.is_some() {
                bail!("{}: saw nested SF before end_of_record", path.display());
            }
            current_path = Some(PathBuf::from(sf));
            current_lines = vec![line.to_owned()];
            current_da = BTreeMap::new();
            continue;
        }

        let Some(record_path) = &current_path else {
            if !line.is_empty() {
                bail!("{}: data before first SF: {line}", path.display());
            }
            continue;
        };

        current_lines.push(line.to_owned());
        if let Some((line_no, count)) = parse_da_line(line)? {
            *current_da.entry(line_no).or_insert(0) += count;
        } else if line == "end_of_record" {
            records.push(LcovRecord {
                path: record_path.clone(),
                lines: std::mem::take(&mut current_lines),
                da_counts: std::mem::take(&mut current_da),
            });
            current_path = None;
        }
    }

    if let Some(record_path) = current_path {
        bail!(
            "{}: missing end_of_record for {}",
            path.display(),
            record_path.display()
        );
    }

    Ok(records)
}

fn parse_da_line(line: &str) -> Result<Option<(u32, i64)>> {
    let Some(rest) = line.strip_prefix("DA:") else {
        return Ok(None);
    };
    let mut parts = rest.split(',');
    let line_no = parts
        .next()
        .context("DA line missing line number")?
        .parse()
        .with_context(|| format!("invalid DA line number: {line}"))?;
    let count = parts
        .next()
        .context("DA line missing hit count")?
        .parse()
        .with_context(|| format!("invalid DA hit count: {line}"))?;
    Ok(Some((line_no, count)))
}

fn parse_function_starts(record: &LcovRecord) -> Result<BTreeMap<String, u32>> {
    let mut starts = BTreeMap::new();
    for line in &record.lines {
        let Some((line_no, name)) = parse_fn_line(line)? else {
            continue;
        };
        if let Some(previous) = starts.insert(name.clone(), line_no) {
            if previous != line_no {
                bail!(
                    "{}: function {name:?} appears at both line {previous} and line {line_no}",
                    record.path.display()
                );
            }
        }
    }
    Ok(starts)
}

#[derive(Debug, Clone)]
struct FunctionRange {
    signature_lines: BTreeSet<u32>,
    executable_body_lines: BTreeSet<u32>,
    terminal_ok_lines: BTreeSet<u32>,
}

fn parse_function_ranges(
    record: &LcovRecord,
    function_starts: &BTreeMap<String, u32>,
) -> Result<BTreeMap<String, FunctionRange>> {
    if record.path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Ok(BTreeMap::new());
    }

    let source = match fs::read_to_string(&record.path) {
        Ok(source) => source,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(err).with_context(|| format!("read {}", record.path.display())),
    };
    let source_lines = source.lines().collect::<Vec<_>>();
    let mut by_start = BTreeMap::<u32, Option<FunctionRange>>::new();
    let mut ranges = BTreeMap::new();

    for (name, &start) in function_starts {
        let range = match by_start.get(&start) {
            Some(range) => range.clone(),
            None => {
                let range = parse_function_range(&record.path, &source_lines, start)?;
                by_start.insert(start, range.clone());
                range
            }
        };
        if let Some(range) = range {
            ranges.insert(name.clone(), range);
        }
    }

    Ok(ranges)
}

fn parse_function_range(
    source_path: &Path,
    source_lines: &[&str],
    start: u32,
) -> Result<Option<FunctionRange>> {
    let Some(start_text) = start
        .checked_sub(1)
        .and_then(|idx| source_lines.get(idx as usize))
        .map(|line| line.trim())
    else {
        bail!(
            "{}:{start}: LCOV function start is past end of file",
            source_path.display()
        );
    };
    if start_text.starts_with("//") || !looks_like_rust_fn_line(start_text) {
        return Ok(None);
    }

    let Some(body) = find_rust_function_body(source_path, source_lines, start)? else {
        return Ok(None);
    };
    let signature_lines = signature_lines_for_body(source_path, source_lines, start, body)?;
    let executable_body_lines = executable_body_lines(source_lines, body)?;
    let terminal_ok_lines = terminal_ok_lines(source_lines, body);
    Ok(Some(FunctionRange {
        signature_lines,
        executable_body_lines,
        terminal_ok_lines,
    }))
}

#[derive(Debug, Clone, Copy)]
struct FunctionBody {
    start_line: u32,
    start_col: usize,
    end_line: u32,
    end_col: usize,
}

fn find_rust_function_body(
    source_path: &Path,
    source_lines: &[&str],
    start: u32,
) -> Result<Option<FunctionBody>> {
    let mut paren_depth = 0i32;
    let last = u32::min(source_lines.len() as u32, start + 80);
    let mut body_start = None;

    for line_no in start..=last {
        let line = source_lines[(line_no - 1) as usize];
        match scan_signature_line(line, paren_depth).with_context(|| {
            format!(
                "{}:{line_no}: invalid Rust function signature",
                source_path.display()
            )
        })? {
            SignatureScan::Continue(depth) => paren_depth = depth,
            SignatureScan::Body { col, depth } => {
                if depth != 0 {
                    bail!(
                        "{}:{line_no}: Rust function body started with nonzero paren depth {depth}",
                        source_path.display()
                    );
                }
                body_start = Some((line_no, col));
                break;
            }
            SignatureScan::DeclarationEnd => return Ok(None),
        }
    }

    let Some((start_line, start_col)) = body_start else {
        bail!(
            "{}:{start}: covered Rust fn line has no body brace within 80 lines",
            source_path.display()
        );
    };

    let (end_line, end_col) =
        find_matching_body_brace(source_path, source_lines, start_line, start_col)?;
    Ok(Some(FunctionBody {
        start_line,
        start_col,
        end_line,
        end_col,
    }))
}

fn find_matching_body_brace(
    source_path: &Path,
    source_lines: &[&str],
    start_line: u32,
    start_col: usize,
) -> Result<(u32, usize)> {
    let mut state = RustLexState::default();
    let mut depth = 1i32;

    for line_no in start_line..=(source_lines.len() as u32) {
        let line = source_lines[(line_no - 1) as usize];
        let start_idx = if line_no == start_line {
            start_col + 1
        } else {
            0
        };
        let code = code_char_indices_after(line, start_idx, &mut state);
        for (idx, ch) in code {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok((line_no, idx));
                    }
                    if depth < 0 {
                        bail!(
                            "{}:{line_no}: Rust function body brace depth went negative",
                            source_path.display()
                        );
                    }
                }
                _ => {}
            }
        }
    }

    bail!(
        "{}:{start_line}: Rust function body has no matching closing brace",
        source_path.display()
    )
}

fn executable_body_lines(source_lines: &[&str], body: FunctionBody) -> Result<BTreeSet<u32>> {
    let mut lines = BTreeSet::new();
    let mut state = RustLexState::default();

    for line_no in body.start_line..=body.end_line {
        let line = source_lines[(line_no - 1) as usize];
        let start_idx = if line_no == body.start_line {
            body.start_col + 1
        } else {
            0
        };
        let end_idx = if line_no == body.end_line {
            body.end_col
        } else {
            line.len()
        };
        let code = code_chars_in_range(line, start_idx, end_idx, &mut state);
        let stripped = code.trim();
        if stripped.is_empty() || is_rust_delimiter_only(stripped) {
            continue;
        }
        lines.insert(line_no);
    }

    Ok(lines)
}

fn signature_lines_for_body(
    source_path: &Path,
    source_lines: &[&str],
    start: u32,
    body: FunctionBody,
) -> Result<BTreeSet<u32>> {
    let mut lines = BTreeSet::new();

    for line_no in start..=body.start_line {
        let line = source_lines[(line_no - 1) as usize];
        if line_no == body.start_line {
            let suffix = strip_line_comment(&line[body.start_col + 1..]).trim();
            if !suffix.is_empty() {
                continue;
            }
        }

        let stripped = strip_line_comment(line).trim();
        if stripped.is_empty() {
            continue;
        }
        if line_no != start && looks_executable(stripped) {
            bail!(
                "{}:{line_no}: refusing to suppress executable-looking line in covered \
                 function signature: {line:?}",
                source_path.display()
            );
        }
        lines.insert(line_no);
    }

    Ok(lines)
}

fn terminal_ok_lines(source_lines: &[&str], body: FunctionBody) -> BTreeSet<u32> {
    let mut lines = BTreeSet::new();

    for line_no in (body.start_line..=body.end_line).rev() {
        let line = source_lines[(line_no - 1) as usize];
        let start_idx = if line_no == body.start_line {
            body.start_col + 1
        } else {
            0
        };
        let end_idx = if line_no == body.end_line {
            body.end_col
        } else {
            line.len()
        };
        let stripped = strip_line_comment(&line[start_idx..end_idx]).trim();
        if stripped.is_empty() || is_rust_delimiter_only(stripped) {
            continue;
        }
        if matches!(stripped, "Ok(())" | "Ok(());") {
            lines.insert(line_no);
        }
        break;
    }

    lines
}

fn infer_sbf_function_hits(
    record: &LcovRecord,
    sbf_lines: Option<&BTreeMap<u32, i64>>,
    function_starts: &BTreeMap<String, u32>,
    function_ranges: &BTreeMap<String, FunctionRange>,
) -> Result<(Vec<String>, usize)> {
    let Some(sbf_lines) = sbf_lines else {
        return Ok((record.lines.clone(), 0));
    };
    if function_starts.is_empty() {
        return Ok((record.lines.clone(), 0));
    }

    let mut lines = Vec::with_capacity(record.lines.len());
    let mut function_hits = BTreeMap::<String, i64>::new();
    let mut inferred = 0usize;
    let mut saw_fnf = false;
    let mut saw_fnh = false;

    for line in &record.lines {
        if let Some((count, name)) = parse_fnda_line(line)? {
            let start_line = function_starts.get(&name).with_context(|| {
                format!(
                    "{}: FNDA record references unknown function {name:?}",
                    record.path.display()
                )
            })?;
            let count = if count == 0
                && function_was_hit_by_sbf(&name, *start_line, sbf_lines, function_ranges)
            {
                inferred += 1;
                1
            } else {
                count
            };
            if function_hits.insert(name.clone(), count).is_some() {
                bail!(
                    "{}: duplicate FNDA record for function {name:?}",
                    record.path.display()
                );
            }
            lines.push(format!("FNDA:{count},{name}"));
        } else {
            match line.as_str() {
                line if line.starts_with("FNF:") => {
                    saw_fnf = true;
                    lines.push(format!("FNF:{}", function_starts.len()));
                }
                line if line.starts_with("FNH:") => {
                    saw_fnh = true;
                    lines.push("__ANCHOR_COVERAGE_FNH__".to_owned());
                }
                _ => lines.push(line.clone()),
            }
        }
    }

    for name in function_starts.keys() {
        if !function_hits.contains_key(name) {
            bail!(
                "{}: FN record for {name:?} has no matching FNDA hit count",
                record.path.display()
            );
        }
    }

    let fnh = function_hits.values().filter(|count| **count > 0).count();
    for line in &mut lines {
        if line == "__ANCHOR_COVERAGE_FNH__" {
            *line = format!("FNH:{fnh}");
        }
    }

    if !saw_fnf || !saw_fnh {
        let end_idx = lines
            .iter()
            .position(|line| line == "end_of_record")
            .context("LCOV record missing end_of_record")?;
        if !saw_fnf {
            lines.insert(end_idx, format!("FNF:{}", function_starts.len()));
        }
        if !saw_fnh {
            lines.insert(end_idx + usize::from(!saw_fnf), format!("FNH:{fnh}"));
        }
    }

    Ok((lines, inferred))
}

fn function_was_hit_by_sbf(
    name: &str,
    start_line: u32,
    sbf_lines: &BTreeMap<u32, i64>,
    function_ranges: &BTreeMap<String, FunctionRange>,
) -> bool {
    if let Some(range) = function_ranges.get(name) {
        return range
            .executable_body_lines
            .iter()
            .any(|line| sbf_lines.contains_key(line));
    }

    sbf_lines.contains_key(&start_line)
}

fn parse_fn_line(line: &str) -> Result<Option<(u32, String)>> {
    let Some(rest) = line.strip_prefix("FN:") else {
        return Ok(None);
    };
    let (line_no, name) = rest
        .split_once(',')
        .with_context(|| format!("FN line missing function name: {line}"))?;
    Ok(Some((
        line_no
            .parse()
            .with_context(|| format!("invalid FN line number: {line}"))?,
        name.to_owned(),
    )))
}

fn parse_fnda_line(line: &str) -> Result<Option<(i64, String)>> {
    let Some(rest) = line.strip_prefix("FNDA:") else {
        return Ok(None);
    };
    let (count, name) = rest
        .split_once(',')
        .with_context(|| format!("FNDA line missing function name: {line}"))?;
    Ok(Some((
        count
            .parse()
            .with_context(|| format!("invalid FNDA hit count: {line}"))?,
        name.to_owned(),
    )))
}

#[derive(Default)]
struct RustLexState {
    block_comment_depth: usize,
    string: Option<StringState>,
    char_literal: bool,
}

enum StringState {
    Normal { escaped: bool },
    Raw { hashes: usize },
}

fn code_chars_in_range(
    line: &str,
    start_idx: usize,
    end_idx: usize,
    state: &mut RustLexState,
) -> String {
    code_char_indices_after(line, start_idx, state)
        .into_iter()
        .take_while(|(idx, _)| *idx < end_idx)
        .map(|(_, ch)| ch)
        .collect()
}

fn code_char_indices_after(
    line: &str,
    start_idx: usize,
    state: &mut RustLexState,
) -> Vec<(usize, char)> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut idx = 0usize;

    while idx < line.len() {
        if idx < start_idx {
            idx = next_char_boundary(line, idx);
            continue;
        }

        if state.block_comment_depth > 0 {
            if bytes.get(idx..idx + 2) == Some(b"/*") {
                state.block_comment_depth += 1;
                idx += 2;
            } else if bytes.get(idx..idx + 2) == Some(b"*/") {
                state.block_comment_depth -= 1;
                idx += 2;
            } else {
                idx = next_char_boundary(line, idx);
            }
            continue;
        }

        if let Some(string) = &mut state.string {
            match string {
                StringState::Normal { escaped } => {
                    let ch = line[idx..].chars().next().expect("valid char boundary");
                    idx += ch.len_utf8();
                    if *escaped {
                        *escaped = false;
                    } else if ch == '\\' {
                        *escaped = true;
                    } else if ch == '"' {
                        state.string = None;
                    }
                }
                StringState::Raw { hashes } => {
                    if raw_string_closes_at(bytes, idx, *hashes) {
                        idx += 1 + *hashes;
                        state.string = None;
                    } else {
                        idx = next_char_boundary(line, idx);
                    }
                }
            }
            continue;
        }

        if state.char_literal {
            let ch = line[idx..].chars().next().expect("valid char boundary");
            idx += ch.len_utf8();
            if ch == '\\' {
                idx = next_char_boundary(line, idx);
            } else if ch == '\'' {
                state.char_literal = false;
            }
            continue;
        }

        if bytes.get(idx..idx + 2) == Some(b"//") {
            break;
        }
        if bytes.get(idx..idx + 2) == Some(b"/*") {
            state.block_comment_depth += 1;
            idx += 2;
            continue;
        }
        if let Some((prefix_len, hashes)) = raw_string_starts_at(bytes, idx) {
            state.string = Some(StringState::Raw { hashes });
            idx += prefix_len;
            continue;
        }
        if bytes.get(idx) == Some(&b'"') || bytes.get(idx..idx + 2) == Some(b"b\"") {
            state.string = Some(StringState::Normal { escaped: false });
            idx += if bytes.get(idx) == Some(&b'b') { 2 } else { 1 };
            continue;
        }
        if char_literal_starts_at(line, idx) {
            state.char_literal = true;
            idx += 1;
            continue;
        }

        let ch = line[idx..].chars().next().expect("valid char boundary");
        out.push((idx, ch));
        idx += ch.len_utf8();
    }

    if state.char_literal {
        // Rust char literals cannot legally span lines. Resetting here avoids
        // treating a later lifetime annotation as part of a phantom char.
        state.char_literal = false;
    }
    out
}

fn next_char_boundary(line: &str, idx: usize) -> usize {
    match line[idx..].chars().next() {
        Some(ch) => idx + ch.len_utf8(),
        None => line.len(),
    }
}

fn raw_string_starts_at(bytes: &[u8], idx: usize) -> Option<(usize, usize)> {
    let mut pos = idx;
    if bytes.get(pos) == Some(&b'b') {
        pos += 1;
    }
    if bytes.get(pos) != Some(&b'r') {
        return None;
    }
    pos += 1;
    let mut hashes = 0usize;
    while bytes.get(pos) == Some(&b'#') {
        hashes += 1;
        pos += 1;
    }
    if bytes.get(pos) != Some(&b'"') {
        return None;
    }
    Some((pos + 1 - idx, hashes))
}

fn raw_string_closes_at(bytes: &[u8], idx: usize, hashes: usize) -> bool {
    if bytes.get(idx) != Some(&b'"') {
        return false;
    }
    (0..hashes).all(|offset| bytes.get(idx + 1 + offset) == Some(&b'#'))
}

fn char_literal_starts_at(line: &str, idx: usize) -> bool {
    if line.as_bytes().get(idx) != Some(&b'\'') {
        return false;
    }
    let before = line[..idx].chars().rev().find(|ch| !ch.is_whitespace());
    if matches!(before, Some(ch) if ch == '&' || ch == '<') {
        return false;
    }
    let Some(end) = line[idx + 1..].find('\'') else {
        return false;
    };
    // A lifetime after a comma (for example `<'a, 'b>`) can have another
    // apostrophe later on the same line. It is not a character literal.
    if before == Some(',')
        && line[idx + 1..idx + 1 + end]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        return false;
    }
    true
}

fn build_source_suppression(records: &[LcovRecord]) -> Result<BTreeMap<PathBuf, BTreeSet<u32>>> {
    let mut suppress = BTreeMap::new();
    for record in records {
        if record.path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        if !should_parse_rust_source_artifacts(&record.path) {
            continue;
        }

        let hit_lines = record
            .da_counts
            .iter()
            .filter_map(|(line, count)| (*count > 0).then_some(*line))
            .collect::<BTreeSet<_>>();
        let mut lines = delimiter_only_lines(&record.path)?;
        if !hit_lines.is_empty() {
            lines.extend(source_artifact_lines(&record.path, &hit_lines)?);
        }
        if !lines.is_empty() {
            suppress.insert(record.path.clone(), lines);
        }
    }
    Ok(suppress)
}

fn should_parse_rust_source_artifacts(path: &Path) -> bool {
    let path = path.to_string_lossy();
    !path.contains("/.cargo/") && !path.contains("/target/") && !path.contains("/rustc/")
}

fn delimiter_only_lines(source_path: &Path) -> Result<BTreeSet<u32>> {
    let source = match fs::read_to_string(source_path) {
        Ok(source) => source,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(err).with_context(|| format!("read {}", source_path.display())),
    };
    Ok(source
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            is_rust_delimiter_only(strip_line_comment(line).trim()).then_some(idx as u32 + 1)
        })
        .collect())
}

fn source_artifact_lines(source_path: &Path, hit_lines: &BTreeSet<u32>) -> Result<BTreeSet<u32>> {
    let source = match fs::read_to_string(source_path) {
        Ok(source) => source,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(err) => return Err(err).with_context(|| format!("read {}", source_path.display())),
    };
    let source_lines = source.lines().collect::<Vec<_>>();
    let mut suppress = BTreeSet::new();

    for (line_idx, line) in source_lines.iter().enumerate() {
        let line_no = line_idx as u32 + 1;
        let start_text = line.trim();
        if start_text.starts_with("//") || !looks_like_rust_fn_line(start_text) {
            continue;
        }

        let Some(range) = parse_function_range(source_path, &source_lines, line_no)? else {
            continue;
        };
        if function_range_was_hit_by_sbf(&range, line_no, hit_lines) {
            suppress.extend(range.signature_lines);
            suppress.extend(range.terminal_ok_lines);
        }
    }

    suppress.extend(macro_continuation_lines(
        source_path,
        &source_lines,
        hit_lines,
    )?);

    Ok(suppress)
}

fn function_range_was_hit_by_sbf(
    range: &FunctionRange,
    start_line: u32,
    hit_lines: &BTreeSet<u32>,
) -> bool {
    hit_lines.contains(&start_line)
        || range
            .executable_body_lines
            .iter()
            .any(|line| hit_lines.contains(line))
}

fn macro_continuation_lines(
    source_path: &Path,
    source_lines: &[&str],
    hit_lines: &BTreeSet<u32>,
) -> Result<BTreeSet<u32>> {
    let mut suppress = BTreeSet::new();

    for &start in hit_lines {
        let Some(line) = start
            .checked_sub(1)
            .and_then(|idx| source_lines.get(idx as usize))
        else {
            continue;
        };
        let Some((open_col, open, close)) = macro_open_delimiter(line) else {
            continue;
        };

        let Some(end) =
            find_matching_delimiter(source_path, source_lines, start, open_col, open, close)?
        else {
            continue;
        };
        if end <= start {
            continue;
        }

        for line_no in (start + 1)..=end {
            let line = source_lines[(line_no - 1) as usize];
            let stripped = strip_line_comment(line).trim();
            if stripped.is_empty() || is_rust_delimiter_only(stripped) {
                continue;
            }
            if looks_executable(stripped) {
                bail!(
                    "{}:{line_no}: refusing to suppress executable-looking line in covered \
                     macro continuation: {line:?}",
                    source_path.display()
                );
            }
            suppress.insert(line_no);
        }
    }

    Ok(suppress)
}

fn macro_open_delimiter(line: &str) -> Option<(usize, char, char)> {
    let bang = line.find('!')?;
    let before = line[..bang].chars().next_back()?;
    if !(before.is_ascii_alphanumeric() || before == '_') {
        return None;
    }
    let after_bang = &line[bang + 1..];
    let mut chars = after_bang
        .char_indices()
        .skip_while(|(_, ch)| ch.is_whitespace());
    let (offset, open) = chars.next()?;
    if !matches!(open, '(' | '[' | '{') {
        return None;
    }
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => unreachable!(),
    };
    Some((bang + 1 + offset, open, close))
}

fn find_matching_delimiter(
    source_path: &Path,
    source_lines: &[&str],
    start_line: u32,
    start_col: usize,
    open: char,
    close: char,
) -> Result<Option<u32>> {
    let mut state = RustLexState::default();
    let mut depth = 0i32;

    for line_no in start_line..=(source_lines.len() as u32) {
        let line = source_lines[(line_no - 1) as usize];
        let start_idx = if line_no == start_line { start_col } else { 0 };
        for (_, ch) in code_char_indices_after(line, start_idx, &mut state) {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(line_no));
                }
                if depth < 0 {
                    bail!(
                        "{}:{line_no}: macro delimiter depth went negative",
                        source_path.display()
                    );
                }
            }
        }
    }

    Ok(None)
}

enum SignatureScan {
    Continue(i32),
    Body { col: usize, depth: i32 },
    DeclarationEnd,
}

fn scan_signature_line(line: &str, paren_depth: i32) -> Result<SignatureScan> {
    let code = strip_line_comment(line);
    let mut depth = paren_depth;
    for (idx, ch) in code.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    bail!("negative paren depth in signature line: {line:?}");
                }
            }
            '{' if depth == 0 => return Ok(SignatureScan::Body { col: idx, depth }),
            ';' if depth == 0 => return Ok(SignatureScan::DeclarationEnd),
            _ => {}
        }
    }
    Ok(SignatureScan::Continue(depth))
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

fn looks_like_rust_fn_line(line: &str) -> bool {
    line.contains("fn ") || line.contains("fn\t")
}

fn looks_executable(line: &str) -> bool {
    ["let ", "return ", "?;", "if ", "match "]
        .iter()
        .any(|token| line.contains(token))
}

fn is_rust_delimiter_only(line: &str) -> bool {
    matches!(
        line,
        ")" | ")," | ");" | "]" | "]," | "];" | "}" | "}," | "};"
    )
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs, tempfile::tempdir};

    #[test]
    fn function_body_parser_ignores_braces_in_char_arrays() {
        let source = [
            "fn trim_delimiters(input: &str) -> &str {",
            "    input.trim_start_matches(['(', '[', '{'])",
            "}",
            "",
            "fn next() {}",
        ];
        let body = find_rust_function_body(
            Path::new("fixture.rs"),
            &source,
            1,
        )
        .unwrap()
        .expect("function should have a body");

        assert_eq!((body.end_line, body.end_col), (3, 0));
    }

    #[test]
    fn filter_host_lcov_removes_only_non_executable_noise() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(
            &source,
            [
                "pub fn hit(",
                "    arg: u8,",
                ") -> Result<()> {",
                "    if arg == 0 {",
                "        return Err(());",
                "    }",
                "    Ok(())",
                "}",
                "",
            ]
            .join("\n"),
        )
        .unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!(
                "SF:{}\nDA:1,3\nDA:4,2\nDA:8,3\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!(
                "SF:{}\nDA:1,0\nDA:2,0\nDA:3,0\nDA:4,0\nDA:5,0\nDA:6,0\nDA:7,0\nDA:8,0\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        filter_host_lcov(&sbf, &host, &output).unwrap();
        let filtered = fs::read_to_string(output).unwrap();

        assert!(
            !filtered.contains("DA:1,0\n"),
            "exact SBF hit should remove host zero"
        );
        assert!(
            !filtered.contains("DA:2,0\n"),
            "signature arg line is non-executable"
        );
        assert!(
            !filtered.contains("DA:3,0\n"),
            "signature terminator line is non-executable"
        );
        assert!(
            !filtered.contains("DA:4,0\n"),
            "exact SBF hit should remove host zero"
        );
        assert!(
            filtered.contains("DA:5,0\n"),
            "real return branch must stay uncovered"
        );
        assert!(
            !filtered.contains("DA:6,0\n"),
            "delimiter-only line is non-executable"
        );
        assert!(
            !filtered.contains("DA:7,0\n"),
            "trivial terminal Ok tail is a source attribution artifact"
        );
        assert!(
            !filtered.contains("DA:8,0\n"),
            "exact SBF hit should remove host zero"
        );
    }

    #[test]
    fn filter_host_lcov_fails_closed_on_executable_signature_continuation() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(
            &source,
            ["pub fn suspicious(", "    let x = 1,", ") {", "}"].join("\n"),
        )
        .unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!("SF:{}\nDA:1,1\nend_of_record\n", source.display()),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!("SF:{}\nDA:2,0\nend_of_record\n", source.display()),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        let err = filter_host_lcov(&sbf, &host, &output).unwrap_err();
        assert!(
            err.to_string()
                .contains("refusing to suppress executable-looking line"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn filter_host_lcov_suppresses_macro_continuation_lines_for_hit_macro() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(
            &source,
            [
                "pub fn check(data: &[u8]) -> Result<(), ProgramError> {",
                "    require_eq!(",
                "        data.len(),",
                "        core::mem::size_of::<Self>(),",
                "        ProgramError::InvalidAccountData",
                "    );",
                "    Ok(())",
                "}",
                "",
            ]
            .join("\n"),
        )
        .unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!("SF:{}\nDA:2,5\nend_of_record\n", source.display()),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!(
                "SF:{}\nDA:1,0\nDA:2,0\nDA:3,0\nDA:4,0\nDA:5,0\nDA:6,0\nDA:7,0\nDA:8,0\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        filter_host_lcov(&sbf, &host, &output).unwrap();
        let filtered = fs::read_to_string(output).unwrap();

        assert!(
            !filtered.contains("DA:3,0\n"),
            "macro argument line is not an independent uncovered line"
        );
        assert!(
            !filtered.contains("DA:4,0\n"),
            "macro argument line is not an independent uncovered line"
        );
        assert!(
            !filtered.contains("DA:5,0\n"),
            "macro error argument line is not branch coverage"
        );
    }

    #[test]
    fn filter_host_lcov_fails_closed_on_executable_macro_continuation() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(
            &source,
            [
                "pub fn check() {",
                "    some_macro!(",
                "        if condition { value } else { other },",
                "    );",
                "}",
            ]
            .join("\n"),
        )
        .unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!("SF:{}\nDA:2,1\nend_of_record\n", source.display()),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!("SF:{}\nDA:3,0\nend_of_record\n", source.display()),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        let err = filter_host_lcov(&sbf, &host, &output).unwrap_err();
        assert!(
            err.to_string()
                .contains("refusing to suppress executable-looking line"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn filter_host_lcov_infers_sbf_function_hits_from_executable_body_lines() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(
            &source,
            [
                "pub fn hit() {",
                "    do_work();",
                "}",
                "",
                "pub fn miss() {",
                "    do_work();",
                "}",
                "",
                "pub fn body_only() -> u64 {",
                "    u64::from_le_bytes([0; 8])",
                "}",
                "",
                "pub fn delimiter_only() {",
                "}",
                "",
            ]
            .join("\n"),
        )
        .unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!(
                "SF:{}\nDA:1,7\nDA:2,9\nDA:10,4\nDA:14,2\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!(
                "SF:{}\nFN:1,_hit\nFN:5,_miss\nFN:9,_body_only\nFN:13,_delimiter_only\nFNDA:0,_hit\nFNDA:0,_miss\nFNDA:0,_body_only\nFNDA:0,_delimiter_only\nFNF:4\nFNH:0\nDA:1,0\nDA:2,0\nDA:3,0\nDA:5,0\nDA:6,0\nDA:7,0\nDA:9,0\nDA:10,0\nDA:11,0\nDA:13,0\nDA:14,0\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        filter_host_lcov(&sbf, &host, &output).unwrap();
        let filtered = fs::read_to_string(output).unwrap();

        assert!(
            filtered.contains("FNDA:1,_hit\n"),
            "function with an SBF-hit executable body line should be marked hit"
        );
        assert!(
            filtered.contains("FNDA:0,_miss\n"),
            "function without an SBF-hit body line must stay uncovered"
        );
        assert!(
            filtered.contains("FNDA:1,_body_only\n"),
            "function should be marked hit when SBF hits the body but not the fn line"
        );
        assert!(
            !filtered.contains("DA:9,0\n"),
            "function declaration line should be removed when SBF hits the body"
        );
        assert!(
            filtered.contains("FNDA:0,_delimiter_only\n"),
            "delimiter-only body hits must not create function hits"
        );
        assert!(filtered.contains("FNF:4\n"));
        assert!(filtered.contains("FNH:2\n"));
    }

    #[test]
    fn filter_host_lcov_function_body_inference_does_not_leak_to_next_function() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(
            &source,
            [
                "pub fn first() {",
                "    not_hit();",
                "}",
                "",
                "pub fn second() {",
                "    hit();",
                "}",
                "",
            ]
            .join("\n"),
        )
        .unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!("SF:{}\nDA:6,3\nend_of_record\n", source.display()),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!(
                "SF:{}\nFN:1,_first\nFN:5,_second\nFNDA:0,_first\nFNDA:0,_second\nFNF:2\nFNH:0\nDA:1,0\nDA:2,0\nDA:3,0\nDA:5,0\nDA:6,0\nDA:7,0\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        filter_host_lcov(&sbf, &host, &output).unwrap();
        let filtered = fs::read_to_string(output).unwrap();

        assert!(
            filtered.contains("FNDA:0,_first\n"),
            "SBF hits in a later function must not mark the previous function hit"
        );
        assert!(
            filtered.contains("FNDA:1,_second\n"),
            "SBF body hit should mark the containing function hit"
        );
        assert!(filtered.contains("FNH:1\n"));
    }

    #[test]
    fn filter_host_lcov_fails_closed_on_unknown_function_hit_record() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("lib.rs");
        fs::write(&source, ["pub fn hit() {", "}"].join("\n")).unwrap();

        let sbf = tmp.path().join("sbf.lcov");
        fs::write(
            &sbf,
            format!("SF:{}\nDA:1,1\nend_of_record\n", source.display()),
        )
        .unwrap();

        let host = tmp.path().join("host.lcov");
        fs::write(
            &host,
            format!(
                "SF:{}\nFN:1,_hit\nFNDA:0,_hit\nFNDA:0,_unknown\nFNF:2\nFNH:0\nend_of_record\n",
                source.display()
            ),
        )
        .unwrap();

        let output = tmp.path().join("filtered.lcov");
        let err = filter_host_lcov(&sbf, &host, &output).unwrap_err();
        assert!(
            format!("{err:?}").contains("unknown function"),
            "unexpected error: {err:?}"
        );
    }
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
