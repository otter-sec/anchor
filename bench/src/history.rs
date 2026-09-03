use {
    anyhow::{bail, Context, Result},
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, fs, path::Path, process::Command},
};

/// File name used to persist benchmark history.
pub const RESULTS_FILE: &str = "results.json";
/// Synthetic commit label used for the latest benchmark snapshot.
pub const CURRENT_COMMIT: &str = "current";
/// Preferred branch name used to validate persisted benchmark snapshots.
/// Once v2 moves to master, this name should be updated.
pub const MASTER_BRANCH: &str = "anchor-next";

/// Stores the benchmark history as an ordered list of snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BenchmarkHistory {
    pub baseline: BTreeMap<String, ProgramBenchmark>,
    pub baseline_programs: BTreeMap<String, Vec<String>>,
    pub results: Vec<BenchmarkResult>,
}

/// Captures benchmark results for a single commit or synthetic snapshot label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub commit: String,
    pub programs: BTreeMap<String, ProgramBenchmark>,
}

/// Records the measured output for one benchmarked program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramBenchmark {
    pub binary_size_bytes: u64,
    pub compute_units: BTreeMap<String, u64>,
}

/// Loads benchmark history from disk using the current JSON schema.
pub fn load_history(results_path: &Path) -> Result<BenchmarkHistory> {
    let contents = match fs::read_to_string(results_path) {
        Ok(contents) => contents,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {}", results_path.display()));
        }
    };

    serde_json::from_str(&contents).with_context(|| {
        format!(
            "failed to parse benchmark history from {}",
            results_path.display()
        )
    })
}

/// Writes the current benchmark history to disk using pretty-printed JSON.
pub fn save_history(results_path: &Path, history: &BenchmarkHistory) -> Result<()> {
    fs::write(
        results_path,
        format!("{}\n", serde_json::to_string_pretty(history)?),
    )
    .with_context(|| format!("failed to write {}", results_path.display()))
}

/// Inserts the latest benchmark snapshot into history using the previous git commit when needed.
pub fn update_history(
    history: &mut BenchmarkHistory,
    current_result: BenchmarkResult,
) -> Result<()> {
    let previous_commit = latest_master_ancestor_commit()?;
    update_history_with_previous_commit(history, current_result, &previous_commit);
    Ok(())
}

/// Ensures every persisted snapshot commit is reachable from the master branch.
pub fn validate_history_commits(history: &BenchmarkHistory) -> Result<()> {
    let master_ref = master_reference()?;

    for result in &history.results {
        if result.commit == CURRENT_COMMIT {
            continue;
        }

        ensure_commit_is_on_master(&result.commit, &master_ref)?;
    }

    Ok(())
}

/// Updates the history with a caller-provided previous commit reference.
fn update_history_with_previous_commit(
    history: &mut BenchmarkHistory,
    current_result: BenchmarkResult,
    previous_commit: &str,
) {
    match history.results.first_mut() {
        Some(existing_current) if existing_current.commit == CURRENT_COMMIT => {
            if benchmark_changed(existing_current, &current_result) {
                existing_current.commit = previous_commit.to_owned();
                history.results.insert(0, current_result);
            } else {
                *existing_current = current_result;
            }
        }
        _ => history.results.insert(0, current_result),
    }
}

/// Returns true when any benchmarked value or benchmark shape differs from the prior snapshot.
fn benchmark_changed(previous: &BenchmarkResult, current: &BenchmarkResult) -> bool {
    previous.programs != current.programs
}

/// Resolves the most recent ancestor of `HEAD` that is present on the master branch.
fn latest_master_ancestor_commit() -> Result<String> {
    let master_ref = master_reference()?;
    let output = Command::new("git")
        .args(["merge-base", "HEAD", &master_ref])
        .output()
        .with_context(|| format!("failed to get latest master ancestor from {master_ref}"))?;

    if !output.status.success() {
        bail!(
            "failed to get latest master ancestor from {master_ref}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8(output.stdout)
        .context("latest master ancestor was not valid UTF-8")?
        .trim()
        .to_owned())
}

/// Returns the local git ref that should be treated as the canonical master branch.
fn master_reference() -> Result<String> {
    for candidate in [MASTER_BRANCH, "origin/master"] {
        let status = Command::new("git")
            .args(["rev-parse", "--verify", candidate])
            .status()
            .with_context(|| format!("failed to check git ref {candidate}"))?;

        if status.success() {
            return Ok(candidate.to_owned());
        }
    }

    bail!("failed to find a `{MASTER_BRANCH}` branch reference")
}

/// Checks that a specific commit is reachable from the selected master branch reference.
fn ensure_commit_is_on_master(commit: &str, master_ref: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["merge-base", "--is-ancestor", commit, master_ref])
        .status()
        .with_context(|| format!("failed to validate commit {commit} against {master_ref}"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("benchmark history commit {commit} is not present in {master_ref}")
    }
}
