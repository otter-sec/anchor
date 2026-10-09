use {
    crate::results::{Results, VersionResult},
    anyhow::{bail, Context, Result},
    indexmap::IndexMap,
    std::{fs, path::Path},
    unicode_width::UnicodeWidthStr,
};

type Values = IndexMap<String, u64>;
type Row = [String; 3];

#[derive(Clone, Copy)]
enum Measurement {
    BinarySize,
    ComputeUnits,
    StackMemory,
}

impl Measurement {
    fn values(self, result: &VersionResult) -> &Values {
        match self {
            Self::BinarySize => &result.result.binary_size,
            Self::ComputeUnits => &result.result.compute_units,
            Self::StackMemory => &result.result.stack_memory,
        }
    }

    fn details(self) -> &'static str {
        match self {
            Self::BinarySize => "Program results",
            Self::ComputeUnits => "Instruction results",
            Self::StackMemory => "Accounts struct results",
        }
    }
}

pub fn sync(directory: &Path, results: &Results) -> Result<()> {
    for (file, measurement) in [
        ("BINARY_SIZE.md", Measurement::BinarySize),
        ("COMPUTE_UNITS.md", Measurement::ComputeUnits),
        ("STACK_MEMORY.md", Measurement::StackMemory),
    ] {
        let path = directory.join(file);
        let mut markdown = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        sync_file(&mut markdown, results, measurement)?;
        fs::write(&path, markdown)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

pub fn bump_version(directory: &Path, version: &str, results: &Results) -> Result<()> {
    let mut files = Vec::new();
    for (file, measurement) in [
        ("BINARY_SIZE.md", Measurement::BinarySize),
        ("COMPUTE_UNITS.md", Measurement::ComputeUnits),
        ("STACK_MEMORY.md", Measurement::StackMemory),
    ] {
        let path = directory.join(file);
        let mut markdown = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        insert_release_section(&mut markdown, version)?;
        sync_file(&mut markdown, results, measurement)?;
        files.push((path, markdown));
    }

    for (path, markdown) in files {
        fs::write(&path, markdown)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

fn insert_release_section(markdown: &mut String, version: &str) -> Result<()> {
    let release_title = format!("## [{version}]");
    if markdown.contains(&release_title) {
        bail!("Markdown already contains an Anchor {version} section");
    }

    let unreleased_title = "## [Unreleased]";
    let start = markdown
        .find(unreleased_title)
        .context("Missing Unreleased section")?;
    let end = markdown[start..]
        .find("\n---")
        .map(|offset| start + offset + "\n---".len())
        .context("Missing separator after Unreleased section")?;
    let release = markdown[start..end].replacen(unreleased_title, &release_title, 1);
    markdown.insert_str(end, &format!("\n\n{release}"));
    Ok(())
}

fn sync_file(markdown: &mut String, results: &Results, measurement: Measurement) -> Result<()> {
    let versions = results.iter().collect::<Vec<_>>();
    let Some(mut old) = versions.first().copied() else {
        return Ok(());
    };

    for (index, new) in versions.into_iter().enumerate() {
        if old.0 == "unreleased" {
            break;
        }
        update_version(
            markdown,
            new.0,
            &new.1.solana_version,
            measurement.values(new.1),
            measurement.values(old.1),
            index == 0,
            measurement.details(),
        )?;
        old = new;
    }
    Ok(())
}

fn update_version(
    markdown: &mut String,
    version: &str,
    solana: &str,
    new: &Values,
    old: &Values,
    first: bool,
    details: &str,
) -> Result<()> {
    let title = if version == "unreleased" {
        "## [Unreleased]".to_owned()
    } else {
        format!("## [{version}]")
    };
    let start = markdown
        .find(&title)
        .with_context(|| format!("Missing {title} section"))?;
    let section_end = markdown[start..]
        .find("\n---")
        .map(|offset| start + offset)
        .with_context(|| format!("Missing separator after {title}"))?;
    let version_start = markdown[start..]
        .find("Solana version: ")
        .map(|offset| start + offset)
        .with_context(|| format!("Missing Solana version in {title}"))?;
    let version_end = markdown[version_start..]
        .find('\n')
        .map(|offset| version_start + offset)
        .with_context(|| format!("Missing newline after Solana version in {title}"))?;
    let results_start = version_end + 1;
    let results_end = markdown[results_start..section_end]
        .find("\n### Notable changes")
        .map(|offset| results_start + offset)
        .unwrap_or(section_end);
    let table_start = markdown[results_start..results_end]
        .find('|')
        .map(|offset| results_start + offset)
        .with_context(|| format!("Missing table in {title}"))?;
    let header_end = markdown[table_start..results_end]
        .find('\n')
        .map(|offset| table_start + offset)
        .with_context(|| format!("Missing table separator in {title}"))?;
    let header = markdown[table_start..header_end].to_owned();
    markdown.replace_range(
        results_start..results_end,
        &format!("\n{}\n", format_results(&header, new, old, first, details)?),
    );

    markdown.replace_range(
        version_start..version_end,
        &format!("Solana version: {solana}"),
    );
    Ok(())
}

fn format_results(
    header: &str,
    new: &Values,
    old: &Values,
    first: bool,
    details: &str,
) -> Result<String> {
    let average = mean(new);
    let average_change = if first {
        "Baseline".into()
    } else if new == old {
        "No change".into()
    } else {
        format_average_change(average, mean(old), new, old)
    };
    let table = format_table(header, new, old, first)?;

    if !first && new == old {
        Ok(format!(
            "<details>\n<summary>No change</summary>\n\n{table}\n</details>",
        ))
    } else {
        Ok(format!(
            "**Average:** {average_change}\n\n<details>\n<summary>{details} \
             ({})</summary>\n\n{table}\n</details>",
            new.len(),
        ))
    }
}

fn mean(values: &Values) -> f64 {
    values.values().sum::<u64>() as f64 / values.len() as f64
}

fn format_average_change(average: f64, old_average: f64, new: &Values, old: &Values) -> String {
    let percentages = new
        .iter()
        .filter_map(|(name, value)| {
            old.get(name)
                .map(|old_value| percent_change(*value, *old_value))
        })
        .collect::<Vec<_>>();
    let range = if percentages.is_empty() {
        "N/A, N/A".into()
    } else {
        let minimum = percentages.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = percentages
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        format!(
            "{}, {}",
            format_signed_percent(minimum),
            format_signed_percent(maximum)
        )
    };
    let percent = percent_change_float(average, old_average);
    let icon = if percent > 0.0 { "🔴" } else { "🟢" };
    let sign = if average > old_average { "+" } else { "" };
    format!(
        "{icon} **{sign}{} ({}%)** [{range}]",
        format_decimal(average - old_average),
        format_percent(percent.abs()),
    )
}

fn percent_change(new: u64, old: u64) -> f64 {
    percent_change_float(new as f64, old as f64)
}

fn percent_change_float(new: f64, old: f64) -> f64 {
    (new / old - 1.0) * 100.0
}

fn format_signed_percent(percent: f64) -> String {
    format!(
        "{}{}%",
        if percent > 0.0 {
            "+"
        } else if percent < 0.0 {
            "-"
        } else {
            ""
        },
        format_percent(percent.abs())
    )
}

fn format_percent(percent: f64) -> String {
    ryu_js::Buffer::new().format_to_fixed(percent, 2).to_owned()
}

fn format_table(header: &str, new: &Values, old: &Values, first: bool) -> Result<String> {
    let headers: Row = header
        .trim_matches(['|', ' '])
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Expected a three-column Markdown table: {header}"))?;
    let mut names = new.keys().collect::<Vec<_>>();
    let mut position = 0;
    // Keep removed results beside their original neighbours, not at the end.
    for name in old.keys() {
        if let Some(index) = names.iter().position(|entry| *entry == name) {
            position = index + 1;
        } else {
            names.insert(position, name);
            position += 1;
        }
    }

    let rows = names
        .into_iter()
        .map(|name| {
            let Some(value) = new.get(name) else {
                return [name.to_owned(), "-".into(), "Removed".into()];
            };
            let change = match old.get(name) {
                None => "N/A".into(),
                Some(previous) if previous == value => if first { "N/A" } else { "-" }.into(),
                Some(previous) => format_change(*value, *previous),
            };
            row(name, *value, change)
        })
        .collect::<Vec<_>>();
    let mut widths = [3; 3];
    for row in std::iter::once(&headers).chain(&rows) {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(UnicodeWidthStr::width(cell.as_str()));
        }
    }
    let mut table = String::new();
    write_row(&mut table, &headers, widths);
    write_row(&mut table, &widths.map(|width| "-".repeat(width)), widths);
    for row in &rows {
        write_row(&mut table, row, widths);
    }
    Ok(table)
}

fn row(name: &str, value: u64, change: String) -> Row {
    [name.to_owned(), format_number(value.into()), change]
}

fn write_row(table: &mut String, row: &Row, widths: [usize; 3]) {
    table.push('|');
    for (cell, width) in row.iter().zip(widths) {
        table.push(' ');
        table.push_str(cell);
        table.extend(std::iter::repeat_n(
            ' ',
            width - UnicodeWidthStr::width(cell.as_str()),
        ));
        table.push_str(" |");
    }
    table.push('\n');
}

fn format_change(new: u64, old: u64) -> String {
    let delta = i128::from(new) - i128::from(old);
    let percent = ryu_js::Buffer::new()
        .format_to_fixed((new as f64 / old as f64 - 1.0) * 100.0, 2)
        .to_owned();
    if percent.parse::<f64>().unwrap_or(f64::NAN) > 0.0 {
        format!("🔴 **+{} ({percent}%)**", format_number(delta))
    } else {
        format!(
            "🟢 **{} ({}%)**",
            format_number(delta),
            percent.strip_prefix('-').unwrap_or(&percent)
        )
    }
}

fn format_number(number: i128) -> String {
    let raw = number.to_string();
    let first_digit = usize::from(number < 0);
    let mut formatted = String::with_capacity(raw.len() + raw.len() / 3);
    for (index, character) in raw.chars().enumerate() {
        if index > first_digit && (raw.len() - index).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted
}

fn format_decimal(number: f64) -> String {
    let raw = ryu_js::Buffer::new().format_to_fixed(number, 2).to_owned();
    let raw = raw.trim_end_matches('0').trim_end_matches('.');
    let (sign, raw) = raw.strip_prefix('-').map_or(("", raw), |raw| ("-", raw));
    let (integer, fraction) = raw.split_once('.').unwrap_or((raw, ""));
    let integer = format_number(integer.parse().expect("formatted number is an integer"));
    if fraction.is_empty() {
        format!("{sign}{integer}")
    } else {
        format!("{sign}{integer}.{fraction}")
    }
}
