mod svg;
pub(crate) mod trace;

use {
    anyhow::Result,
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    },
};

/// Build per-transaction flamegraph SVGs for a test, aggregating
/// every program's invocations (top-level + CPIs) into one flamegraph
/// per tx. Output paths: `<output_dir>/<test_name>__tx<N>.svg`.
///
/// `programs` maps program_id (base58) → deployed ELF path. Programs
/// missing from the map still get frames, just labeled as
/// `[unresolved <pid>]`.
///
/// Returns the SVG paths that were actually written.
pub fn render_per_tx_flamegraphs(
    test_name: &str,
    trace_dir: &Path,
    programs: &BTreeMap<String, PathBuf>,
    output_dir: &Path,
    manifest_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let reports = trace::build_tx_reports(test_name, trace_dir, programs, manifest_dir)?;
    if reports.is_empty() {
        return Ok(Vec::new());
    }

    fs::create_dir_all(output_dir)?;
    let mut written = Vec::new();
    for (tx_seq, report) in &reports {
        let path = output_dir.join(format!("{test_name}__tx{tx_seq}.svg"));
        fs::write(&path, svg::render(report))?;
        written.push(path);
    }
    Ok(written)
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
    fn render_per_tx_flamegraphs_writes_one_svg_per_transaction() {
        let dir = tempdir().unwrap();
        let trace_dir = dir.path().join("test_trace");
        let out_dir = dir.path().join("out");
        write_invocation(&trace_dir, "0001__tx1", "Program1111111111111111111", &[0]);
        write_invocation(&trace_dir, "0002__tx2", "Program1111111111111111111", &[0]);

        let written =
            render_per_tx_flamegraphs("smoke", &trace_dir, &BTreeMap::new(), &out_dir, None)
                .unwrap();

        assert_eq!(written.len(), 2);
        assert_eq!(written[0].file_name().unwrap(), "smoke__tx1.svg");
        assert_eq!(written[1].file_name().unwrap(), "smoke__tx2.svg");
        assert!(written.iter().all(|path| path.exists()));
    }
}
