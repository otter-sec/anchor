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

#[cfg(test)]
mod tests {
    use {super::*, std::collections::BTreeMap};

    #[test]
    fn render_outputs_svg_with_title_subtitle_and_cu_label() {
        let report = FlamegraphReport {
            program_name: "demo · tx1".to_string(),
            total_cu: 7,
            stacks: BTreeMap::from([(
                vec![
                    "[program demo]".to_string(),
                    "entry @ 0x0".to_string(),
                    "handler @ 0x4".to_string(),
                ],
                7,
            )]),
        };

        let svg = render(&report);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("demo · tx1 flamegraph"));
        assert!(svg.contains("Approximate CU: 7"));
        assert!(svg.contains("entry @ 0x0"));
        assert!(svg.contains("CU"));
    }
}
