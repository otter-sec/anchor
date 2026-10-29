//! Historical suites reuse Anchor's ordinary Surfpool fixture compiler.
//! Each test starts its own fork from the suite's launch plan, choosing its
//! boundary in code. Surfpool and SVM Replay own state, execution and time.
mod capabilities;
mod plan;
#[cfg(test)]
mod tests;

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use {
    crate::config::{Config, SurfpoolConfig, TestValidator, ValidatorType, WithPath},
    anchor_client::Cluster,
    anyhow::{bail, Context, Result},
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant},
    },
};

/// Environment variable naming the suite's launch plan for the generated test
/// helpers.
pub(crate) const PLAN_ENV: &str = "ANCHOR_HISTORICAL_PLAN";

/// The Anchor.toml whose `[historical]` table holds provider settings, when
/// opted in.
pub(crate) fn config_path(config: &WithPath<Config>) -> Result<Option<PathBuf>> {
    match &config.historical {
        None => Ok(None),
        Some(_) => config
            .path()
            .canonicalize()
            .context("resolve historical provider config")
            .map(Some),
    }
}

/// The launch plan and registry used to reap forks after the test script exits.
#[derive(Debug)]
pub(crate) struct SuitePlan {
    pub(crate) path: PathBuf,
    registry: PathBuf,
    shutdown_wait: Duration,
}

/// A historical suite never predeploys: its programs are deployed into each
/// fork.
pub(crate) fn guard_predeploy(cfg: &WithPath<Config>, predeploy: bool) -> Result<()> {
    if predeploy && cfg.historical.is_some() {
        bail!(
            "historical tests deploy into their own forks; run `anchor test` against localnet \
             without --skip-local-validator so nothing is deployed to another cluster"
        );
    }
    Ok(())
}

/// Writes the suite's launch plan when `[historical]` is configured; ordinary
/// suites get `None`
/// and keep their unchanged validator path.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_suite(
    cfg: &WithPath<Config>,
    suite: &Path,
    test_validator: &Option<TestValidator>,
    surfpool_config: &Option<SurfpoolConfig>,
    validator_type: ValidatorType,
    _skip_deploy: bool,
    detach: bool,
) -> Result<Option<SuitePlan>> {
    let Some(provider_config) = config_path(cfg)? else {
        return Ok(None);
    };
    if cfg!(windows) {
        bail!(
            "UNSUPPORTED_HISTORICAL_PLATFORM: historical tests are not qualified on Windows; SVM \
             Replay runtime support and fork-process cleanup require qualification"
        );
    }
    if validator_type != ValidatorType::Surfpool {
        bail!("historical tests run on Surfpool; remove `--validator legacy`");
    }
    if detach {
        bail!("--detach keeps a local validator running; historical forks belong to the tests");
    }
    if cfg.provider.cluster != Cluster::Localnet {
        bail!("historical tests use their own forks; set [provider] cluster = \"localnet\"");
    }
    plan::refuse_conflicts(surfpool_config, test_validator)?;
    capabilities::require_surfpool()?;
    let generated = crate::generated_validator_accounts(cfg, test_validator)?;
    let normal = crate::surfpool_flags(
        cfg,
        test_validator,
        surfpool_config,
        false,
        // Fork setup never runs automatic deployments against a frozen boundary.
        // Tests install/deploy inside the fork after choosing its boundary.
        true,
        Some(suite),
        &generated,
    )?;
    let root = cfg
        .path()
        .parent()
        .context("Anchor.toml has no parent directory")?
        .canonicalize()?;
    let suite = suite.canonicalize()?;
    let directory = owned_directory(&root.join(".anchor").join("historical"))?;
    let registry = directory.join("forks");
    let mut registry_builder = fs::DirBuilder::new();
    #[cfg(unix)]
    registry_builder.mode(0o700);
    registry_builder.create(&registry)?;
    let shutdown_wait = surfpool_config
        .as_ref()
        .map(|config| config.shutdown_wait)
        .unwrap_or(crate::config::SHUTDOWN_WAIT)
        .max(0) as u64;
    let plan = plan::document(plan::Inputs {
        root: &root,
        suite: &suite,
        args: plan::fixture_args(&normal)?,
        provider_config: &provider_config,
        wallet: &cfg.provider.wallet.to_string(),
        idl_directory: &crate::target_dir()?.join("idl"),
        generated: &generated,
        registry: &registry,
        surfpool_config,
        shutdown_wait,
    })?;
    let path = directory.join("plan.json");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    use std::io::Write;
    options
        .open(&path)?
        .write_all((serde_json::to_string_pretty(&plan)? + "\n").as_bytes())?;
    Ok(Some(SuitePlan {
        path,
        registry,
        shutdown_wait: Duration::from_millis(shutdown_wait),
    }))
}

/// A new directory owned by this suite run; an existing one is never reused or
/// removed.
fn owned_directory(parent: &Path) -> Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fs::create_dir_all(parent)?;
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let directory = parent.join(format!(
        "suite-{}-{started}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    builder
        .create(&directory)
        .with_context(|| format!("create historical suite directory {}", directory.display()))?;
    Ok(directory)
}

/// Terminates recorded forks still alive after the test script exits, such as
/// those of a
/// SIGKILLed test process, and fails the suite when any were found. A fork is
/// matched by its
/// PID and its unique `--surfnet-id` argument, so a reused PID is never
/// signalled. A record is
/// removed only once its fork is verifiably gone; an inspection failure keeps
/// it and fails.
/// If `anchor test` itself is killed, its forks are left to the job or cgroup
/// teardown.
pub(crate) fn reap(plan: Option<&SuitePlan>) -> Result<()> {
    let Some(plan) = plan else {
        return Ok(());
    };
    let mut survivors = Vec::new();
    for entry in fs::read_dir(&plan.registry)? {
        let path = entry?.path();
        let record: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("historical fork record {}", path.display()))?;
        let (Some(pid), Some(surfnet_id)) = (record["pid"].as_u64(), record["surfnetId"].as_str())
        else {
            bail!("invalid historical fork record {}", path.display());
        };
        if is_fork(pid, surfnet_id)? {
            terminate(pid, surfnet_id, plan.shutdown_wait)?;
            survivors.push(format!("pid {pid} ({surfnet_id})"));
        }
        fs::remove_file(&path)?;
    }
    if !survivors.is_empty() {
        bail!(
            "historical forks outlived their tests and were terminated: {}",
            survivors.join(", ")
        );
    }
    Ok(())
}

/// Whether `pid` is alive and still the fork started with `--surfnet-id
/// surfnet_id`.
fn is_fork(pid: u64, surfnet_id: &str) -> Result<bool> {
    let output = Command::new("ps")
        .args(["-ww", "-o", "args=", "-p", &pid.to_string()])
        .output()
        .context("inspect historical fork processes with `ps`")?;
    let args = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        // `ps -p` exits 1 with no output when the process does not exist.
        if output.status.code() == Some(1) && args.trim().is_empty() && output.stderr.is_empty() {
            return Ok(false);
        }
        bail!(
            "`ps` could not inspect historical fork {pid}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(args
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|pair| pair == ["--surfnet-id", surfnet_id]))
}

/// SIGTERM, then SIGKILL after the shutdown wait; fails unless the fork is
/// verifiably gone.
fn terminate(pid: u64, surfnet_id: &str, wait: Duration) -> Result<()> {
    let signal = |name: &str| -> Result<()> {
        Command::new("kill")
            .args([name, &pid.to_string()])
            .status()
            .with_context(|| format!("signal historical fork {pid}"))?;
        Ok(())
    };
    let gone_within = |limit: Duration| -> Result<bool> {
        let deadline = Instant::now() + limit;
        loop {
            if !is_fork(pid, surfnet_id)? {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            thread::sleep(Duration::from_millis(50));
        }
    };
    signal("-TERM")?;
    if gone_within(wait.max(Duration::from_secs(5)))? {
        return Ok(());
    }
    signal("-KILL")?;
    if gone_within(Duration::from_secs(5))? {
        return Ok(());
    }
    bail!("historical fork {pid} ({surfnet_id}) survived SIGKILL")
}
