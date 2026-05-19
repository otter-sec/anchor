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
