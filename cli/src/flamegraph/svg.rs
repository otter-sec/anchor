//! SVG rendering for [`FlamegraphReport`] via the `inferno` crate.
//!
//! We hand Brendan Gregg's folded-stack format (`a;b;c <count>`) to
//! `inferno::flamegraph::from_lines`, which emits the standard
//! interactive flamegraph SVG: click to zoom, reset button, hover
//! tooltips with percentages, built-in search box.

use {
    super::trace::FlamegraphReport,
    inferno::flamegraph::{from_lines, Options},
};

/// Render a report as an interactive SVG (string-typed for easy writing).
pub fn render(report: &FlamegraphReport) -> String {
    let lines: Vec<String> = report
        .stacks
        .iter()
        .map(|(stack, count)| format!("{} {}", stack.join(";"), count))
        .collect();

    let mut opts = Options::default();
    opts.title = format!("{} flamegraph", report.program_name);
    opts.subtitle = Some(format!(
        "Approximate CU: {} (BPF insns × 1 + ComputeBudget syscall base costs; variable syscall \
         costs underestimated)",
        report.total_cu
    ));
    opts.count_name = "CU".to_string();
    opts.font_type = "monospace".to_string();
    opts.hash = true; // deterministic coloring by frame name

    let mut out: Vec<u8> = Vec::new();
    if let Err(err) = from_lines(&mut opts, lines.iter().map(String::as_str), &mut out) {
        // On failure, fall back to an empty placeholder SVG so the render
        // pipeline doesn't blow up the whole test command.
        return error_svg(&format!("flamegraph render failed: {err}"));
    }
    String::from_utf8(out)
        .unwrap_or_else(|err| error_svg(&format!("invalid UTF-8 from inferno: {err}")))
}

fn error_svg(msg: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"800\" height=\"60\">\n<text x=\"8\" \
         y=\"30\" font-family=\"monospace\" fill=\"#b00\">{}</text>\n</svg>\n",
        msg.replace('<', "&lt;").replace('&', "&amp;")
    )
}
