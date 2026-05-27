#![cfg(not(windows))]

//! Golden fixture for `anchor debugger`'s DWARF-backed symbol resolver.
//!
//! Builds a tiny SBF program with full debug info, then asserts that
//! `SourceResolver` maps PCs back to the marker lines in the fixture's
//! `src/lib.rs`. Catches regressions in the `vaddr = text_addr + pc * 8`
//! formula, DWARF parsing, and the unstripped-binary discovery path.
//!
//! Skipped (not failed) when `cargo-build-sbf` is absent — local devs
//! without the Solana toolchain don't get a spurious failure, and CI
//! jobs that pin the toolchain pick it up automatically.

use {
    anchor_cli::debugger::{arena, source::SourceResolver},
    std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
    tempfile::tempdir,
};

const FIXTURE_CRATE_REL: &str = "tests/fixtures/debugger_program";
const FIXTURE_SO_NAME: &str = "debugger_fixture.so";
const MARKER_TAG: &str = "// MARKER:";
const TOOLS_VERSION: &str = "v1.52";
/// PCs beyond any plausible fixture text section. The fixture's `.text`
/// is ~1-2 KB (~250 insns) — 10k gives comfortable headroom without
/// slowing the scan. Out-of-range PCs resolve to `None` and cost pennies.
const MAX_PROBE_PC: u64 = 10_000;
/// DWARF maps each function to several line records (prologue, body,
/// epilogue). A ±2 window covers the marker line plus its immediate
/// neighbors without overlapping adjacent functions (markers are ≥5
/// lines apart in the fixture).
const MARKER_LINE_WINDOW: u32 = 2;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_CRATE_REL)
}

fn cargo_build_sbf_available() -> bool {
    match Command::new("cargo-build-sbf").arg("--version").status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!(
                "skipping: `cargo-build-sbf --version` failed (exit {:?})",
                status.code()
            );
            false
        }
        Err(e) => {
            eprintln!("skipping: `cargo-build-sbf` unavailable ({e})");
            false
        }
    }
}

/// Attempt to build the fixture. Returns `None` when `cargo-build-sbf`
/// is unavailable (local dev without the Solana toolchain); the test
/// should treat that as a skip, not a failure.
fn build_fixture() -> Option<PathBuf> {
    if !cargo_build_sbf_available() {
        return None;
    }

    let fixture = fixture_dir();
    let spawn = Command::new("cargo")
        .args(["build-sbf", "--tools-version", TOOLS_VERSION])
        .env("CARGO_PROFILE_RELEASE_DEBUG", "2")
        .current_dir(&fixture)
        .status();
    let status = match spawn {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping: `cargo build-sbf` unavailable ({e})");
            return None;
        }
    };
    assert!(
        status.success(),
        "`cargo build-sbf` failed (exit {:?})",
        status.code()
    );

    let unstripped = fixture
        .join("target/sbpf-solana-solana/release")
        .join(FIXTURE_SO_NAME);
    assert!(
        unstripped.exists(),
        "expected unstripped build at {}",
        unstripped.display()
    );
    Some(unstripped)
}

/// Scrape `// MARKER: <name>` tags out of the fixture source so the
/// test doesn't hard-code line numbers — editing the fixture to add or
/// reorder markers doesn't break the assertions.
fn marker_lines() -> Vec<(String, u32)> {
    let src =
        std::fs::read_to_string(fixture_dir().join("src/lib.rs")).expect("read fixture lib.rs");
    src.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let tag_at = line.find(MARKER_TAG)?;
            // Reject tags inside doc comments / prose — markers are
            // end-of-line annotations on real code, so the prefix
            // before the tag must contain something other than a
            // comment-opener.
            let prefix = &line[..tag_at];
            let prefix_trimmed = prefix.trim();
            if prefix_trimmed.is_empty()
                || prefix_trimmed.starts_with("//!")
                || prefix_trimmed.starts_with("///")
                || prefix_trimmed == "//"
            {
                return None;
            }
            let name = line[tag_at + MARKER_TAG.len()..].trim().to_owned();
            Some((name, (i + 1) as u32))
        })
        .collect()
}

/// A resolved path counts as "the fixture's lib.rs" if its final two
/// components are `src/lib.rs`. SBF DWARF omits `DW_AT_comp_dir`, so
/// paths come back relative; filtering on the two-component suffix
/// rejects `core/src/lib.rs` (stdlib) while accepting `src/lib.rs`.
fn is_fixture_lib_rs(p: &Path) -> bool {
    let mut comps = p.components().rev();
    let last = comps.next().and_then(|c| c.as_os_str().to_str());
    let penult = comps.next().and_then(|c| c.as_os_str().to_str());
    matches!((penult, last), (Some("src"), Some("lib.rs")))
}

fn regs_bytes(pcs: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pcs.len() * 12 * std::mem::size_of::<u64>());
    for pc in pcs {
        let mut regs = [0u64; 12];
        regs[11] = *pc;
        for reg in regs {
            out.extend_from_slice(&reg.to_le_bytes());
        }
    }
    out
}

fn write_trace_invocation(test_dir: &Path, stem: &str, program_id: &str, pcs: &[u64]) {
    fs::create_dir_all(test_dir).unwrap();
    fs::write(test_dir.join(format!("{stem}.regs")), regs_bytes(pcs)).unwrap();
    fs::write(
        test_dir.join(format!("{stem}.insns")),
        vec![0u8; pcs.len() * 8],
    )
    .unwrap();
    fs::write(test_dir.join(format!("{stem}.program_id")), program_id).unwrap();
}

#[test]
fn source_resolver_maps_pcs_to_fixture_markers() {
    let Some(elf) = build_fixture() else {
        return;
    };

    let resolver = SourceResolver::from_elf_path(&elf);
    assert!(
        !resolver.is_empty(),
        "resolver empty — DWARF did not load from {}",
        elf.display()
    );

    let mut fixture_lines: BTreeSet<u32> = BTreeSet::new();
    for pc in 0..MAX_PROBE_PC {
        if let Some(loc) = resolver.resolve(pc) {
            if is_fixture_lib_rs(&loc.file) {
                fixture_lines.insert(loc.line);
            }
        }
    }

    assert!(
        !fixture_lines.is_empty(),
        "no PCs resolved to the fixture's src/lib.rs — DWARF file table or text_addr arithmetic \
         is likely broken"
    );

    let markers = marker_lines();
    assert!(
        !markers.is_empty(),
        "fixture source has no MARKER tags — test setup is broken"
    );

    for (name, expected_line) in &markers {
        let lo = expected_line.saturating_sub(MARKER_LINE_WINDOW);
        let hi = expected_line + MARKER_LINE_WINDOW;
        let hit = fixture_lines.range(lo..=hi).next().is_some();
        assert!(
            hit,
            "marker `{name}` at line {expected_line} has no PC resolving within \
             ±{MARKER_LINE_WINDOW} lines. Resolved fixture lines: {:?}",
            fixture_lines
        );
    }
}

#[test]
fn source_resolver_enumerates_executable_fixture_lines() {
    let Some(elf) = build_fixture() else {
        return;
    };

    let resolver = SourceResolver::from_elf_path(&elf);
    let fixture_lines: BTreeSet<u32> = resolver
        .executable_lines()
        .into_iter()
        .filter(|loc| is_fixture_lib_rs(&loc.file))
        .map(|loc| loc.line)
        .collect();

    assert!(
        !fixture_lines.is_empty(),
        "no executable lines resolved for the fixture's src/lib.rs — LCOV would only be able to \
         report executed-line maps"
    );

    for (name, expected_line) in marker_lines() {
        let lo = expected_line.saturating_sub(MARKER_LINE_WINDOW);
        let hi = expected_line + MARKER_LINE_WINDOW;
        assert!(
            fixture_lines.range(lo..=hi).next().is_some(),
            "marker `{name}` at line {expected_line} is missing from executable-line set: {:?}",
            fixture_lines
        );
    }
}

#[test]
fn source_resolver_handles_stripped_elf_without_dwarf() {
    let Some(unstripped) = build_fixture() else {
        return;
    };
    // `cargo build-sbf` also produces a stripped `.so` at target/deploy/.
    // The resolver should treat it as "no DWARF" rather than panic.
    let stripped = unstripped
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("walk up to target/")
        .join("deploy")
        .join(FIXTURE_SO_NAME);
    assert!(
        stripped.exists(),
        "expected stripped build at {}",
        stripped.display()
    );

    let resolver = SourceResolver::from_elf_path(&stripped);
    // cargo-build-sbf strips `.debug_*` but keeps `.text`, so the
    // loader builds successfully; every `resolve()` should then return
    // None. Either outcome (empty resolver, or non-empty resolver that
    // returns None everywhere) preserves the TUI's "no source info"
    // fallback — what matters is that nothing panics.
    for pc in 0..MAX_PROBE_PC {
        let _ = resolver.resolve(pc);
    }
}

#[test]
fn debugger_session_orders_invocations_top_down_and_filters_tests() {
    let Some(elf) = build_fixture() else {
        return;
    };

    let dir = tempdir().unwrap();
    let wanted = dir.path().join("wanted_case");
    let ignored = dir.path().join("ignored_case");
    let child_pid = "Child111111111111111111111111111111111";
    let top_pid = "Top11111111111111111111111111111111111";
    let ignored_pid = "Ignored111111111111111111111111111111";

    // The profile callback writes nested invocations bottom-up. The debugger
    // reverses them so users see top-level first, then CPI children.
    write_trace_invocation(&wanted, "0001__tx1", child_pid, &[0, 1]);
    write_trace_invocation(&wanted, "0002__tx1", top_pid, &[0, 1]);
    write_trace_invocation(&ignored, "0001__tx1", ignored_pid, &[0]);

    let programs = BTreeMap::from([
        (child_pid.to_string(), elf.clone()),
        (top_pid.to_string(), elf.clone()),
        (ignored_pid.to_string(), elf),
    ]);

    let session = arena::build_session(
        dir.path(),
        &programs,
        Some(&fixture_dir()),
        Some(&fixture_dir()),
        Some("wanted"),
    )
    .unwrap();

    assert_eq!(session.txs.len(), 1);
    let tx = &session.txs[0];
    assert_eq!(tx.test_name, "wanted_case");
    assert_eq!(tx.tx_seq, 1);
    assert_eq!(tx.nodes.len(), 2);
    assert_eq!(tx.nodes[0].program_id, top_pid);
    assert_eq!(tx.nodes[1].program_id, child_pid);
    assert!(tx.nodes.iter().all(|node| !node.steps.is_empty()));
    assert!(tx.total_cu > 0);
}
