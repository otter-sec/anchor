use {crate::results::VersionResult, indexmap::IndexMap, std::collections::BTreeSet};

const THRESHOLD_PERCENT: u64 = 1;

#[derive(Default)]
pub struct Comparison {
    pub lines: Vec<String>,
    pub exceeds_threshold: bool,
}

impl VersionResult {
    pub fn compare(&self, previous: Option<&Self>) -> Comparison {
        let Some(previous) = previous else {
            return Comparison {
                lines: vec!["  <missing> -> new benchmark".into()],
                exceeds_threshold: true,
            };
        };

        let mut comparison = Comparison::default();
        for (path, old, new) in [
            (
                "solanaVersion",
                previous.solana_version.as_str(),
                self.solana_version.as_str(),
            ),
            (
                "platformToolsVersion",
                previous.platform_tools_version.as_str(),
                self.platform_tools_version.as_str(),
            ),
            (
                "sbpfVersion",
                previous.sbpf_version.as_str(),
                self.sbpf_version.as_str(),
            ),
        ] {
            if old != new {
                comparison.lines.push(format!("  {path}: {old} -> {new}"));
            }
        }
        for (path, old, new) in [
            (
                "result.binarySize",
                &previous.result.binary_size,
                &self.result.binary_size,
            ),
            (
                "result.computeUnits",
                &previous.result.compute_units,
                &self.result.compute_units,
            ),
            (
                "result.stackMemory",
                &previous.result.stack_memory,
                &self.result.stack_memory,
            ),
        ] {
            compare_measurements(&mut comparison, path, old, new);
        }
        comparison
    }
}

fn compare_measurements(
    comparison: &mut Comparison,
    path: &str,
    previous: &IndexMap<String, u64>,
    current: &IndexMap<String, u64>,
) {
    let names = previous
        .keys()
        .chain(current.keys())
        .collect::<BTreeSet<_>>();
    for name in names {
        let field = format!("{path}.{name}");
        match (previous.get(name), current.get(name)) {
            (Some(previous), Some(current)) if previous != current => {
                let difference = previous.abs_diff(*current);
                comparison.exceeds_threshold |= *previous == 0
                    || u128::from(difference) * 100
                        > u128::from(*previous) * u128::from(THRESHOLD_PERCENT);
                let change = if *previous == 0 {
                    "n/a".into()
                } else {
                    let sign = if current > previous { "+" } else { "-" };
                    format!("{sign}{:.2}%", difference as f64 * 100.0 / *previous as f64)
                };
                comparison
                    .lines
                    .push(format!("  {field}: {previous} -> {current} ({change})"));
            }
            (Some(previous), None) => {
                comparison
                    .lines
                    .push(format!("  {field}: {previous} -> <missing>"));
                comparison.exceeds_threshold = true;
            }
            (None, Some(current)) => {
                comparison
                    .lines
                    .push(format!("  {field}: <missing> -> {current}"));
                comparison.exceeds_threshold = true;
            }
            _ => {}
        }
    }
}
