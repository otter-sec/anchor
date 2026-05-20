//! Post-run flamegraph rendering for per-test trace directories.
//!
//! Consumes the layout produced by `anchor-v2-testing`'s profile
//! callback and emits one SVG per *transaction*, aggregating every
//! program's invocations (top-level + CPIs) against the right ELF.

use {
    crate::flamegraph::render_per_tx_flamegraphs,
    anyhow::{Context, Result},
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    },
};

/// Default root inspected by [`render_all_tests`]. Matches the default
/// in `anchor-v2-testing::profile`. Override both with
/// `ANCHOR_PROFILE_DIR`.
pub const DEFAULT_PROFILE_DIR: &str = "target/anchor-v2-profile";

/// Rendered output for one test's worth of traces.
pub struct RenderedTest {
    pub test_name: String,
    /// One SVG path per outer transaction, in tx order.
    pub svg_paths: Vec<PathBuf>,
}

/// Walk `<root>/` for per-test trace directories and render per-tx
/// flamegraph SVGs. Output lands next to the trace directories:
/// `<root>/<test_name>__tx<N>.svg`.
///
/// `programs` maps program_id (base58) → deployed ELF path. Any
/// program_id seen in traces but not in the map gets `[unresolved
/// <pid>]` frames so its CUs aren't silently dropped.
pub fn render_all_tests(
    root: &Path,
    manifest_dir: Option<&Path>,
    programs: &BTreeMap<String, PathBuf>,
) -> Result<Vec<RenderedTest>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();

    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read profile root {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(test_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let svg_paths = render_per_tx_flamegraphs(test_name, &path, programs, root, manifest_dir)
            .with_context(|| format!("render flamegraphs for test {test_name}"))?;

        if svg_paths.is_empty() {
            continue;
        }

        out.push(RenderedTest {
            test_name: test_name.to_owned(),
            svg_paths,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::flamegraph::trace::{INSN_ENTRY_SIZE, REGS_ENTRY_SIZE},
        tempfile::tempdir,
    };

    fn regs_bytes(pcs: &[u64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(pcs.len() * REGS_ENTRY_SIZE);
        for pc in pcs {
            let mut regs = [0u64; 12];
            regs[11] = *pc;
            for reg in regs {
                out.extend_from_slice(&reg.to_le_bytes());
            }
        }
        out
    }

    fn write_invocation(dir: &Path, stem: &str, program_id: &str, pcs: &[u64]) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(format!("{stem}.regs")), regs_bytes(pcs)).unwrap();
        fs::write(
            dir.join(format!("{stem}.insns")),
            vec![0; pcs.len() * INSN_ENTRY_SIZE],
        )
        .unwrap();
        fs::write(dir.join(format!("{stem}.program_id")), program_id).unwrap();
    }

    #[test]
    fn render_all_tests_skips_empty_trace_dirs_and_returns_written_paths() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("empty_test")).unwrap();
        write_invocation(
            &dir.path().join("with_trace"),
            "0001__tx1",
            "Program1111111111111111111",
            &[0],
        );

        let rendered = render_all_tests(dir.path(), None, &BTreeMap::new()).unwrap();

        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].test_name, "with_trace");
        assert_eq!(rendered[0].svg_paths.len(), 1);
        assert!(rendered[0].svg_paths[0].exists());
    }
}
