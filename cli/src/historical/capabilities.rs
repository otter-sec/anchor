use {
    anyhow::{bail, Context, Result},
    std::{
        io::{Read, Seek, SeekFrom},
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    },
};

/// Check the selected local binary, not a release number or a remote service.
pub(super) fn require_surfpool() -> Result<()> {
    check(Command::new("surfpool"), Duration::from_secs(5))
}

fn check(mut command: Command, timeout: Duration) -> Result<()> {
    let program = command.get_program().to_owned();
    let mut output = tempfile::tempfile().context("create Surfpool capability output")?;
    let mut child = command
        .args(["start", "--help"])
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(output.try_clone()?)
        .spawn()
        .with_context(|| format!("SURFPOOL_UNAVAILABLE: cannot run {program:?} start --help"))?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .context("inspect Surfpool capability check")?
        {
            break status;
        }
        if Instant::now() >= deadline {
            child
                .kill()
                .context("stop timed-out Surfpool capability check")?;
            child
                .wait()
                .context("reap timed-out Surfpool capability check")?;
            bail!(
                "SURFPOOL_INCOMPATIBLE: {program:?} start --help did not finish within {} ms",
                timeout.as_millis()
            );
        }
        thread::sleep(Duration::from_millis(10));
    };
    if output.metadata()?.len() > 256 * 1024 {
        bail!("SURFPOOL_INCOMPATIBLE: {program:?} returned oversized start --help output");
    }
    output.seek(SeekFrom::Start(0))?;
    let mut help = String::new();
    output.read_to_string(&mut help)?;
    let required = [
        "--after",
        "--historical-config",
        "--snapshot",
        "--surfnet-id",
    ];
    let missing: Vec<_> = required
        .into_iter()
        .filter(|flag| {
            !help
                .split_ascii_whitespace()
                .any(|word| word.trim_end_matches(',') == *flag)
        })
        .collect();
    if !status.success() || !missing.is_empty() {
        bail!(
            "SURFPOOL_INCOMPATIBLE: selected {program:?} lacks historical launch support (start \
             --help status {status}; missing {}); install a compatible Surfpool build and check \
             which surfpool is selected on PATH",
            missing.join(", ")
        );
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn script(body: &str) -> Command {
        // No writable executable file: concurrent process creation can retain
        // another thread's writer FD briefly and make exec fail with ETXTBSY.
        let mut command = Command::new("sh");
        command.args(["-c", body, "surfpool"]);
        command
    }

    #[test]
    fn accepts_the_local_launch_capabilities_without_starting_a_server() {
        let command = script(
            "test \"$#\" = 2 && test \"$1\" = start && test \"$2\" = --help || exit 42\nprintf \
             '%s\\n' '--after <BOUNDARY>' '--historical-config <PATH>' '--snapshot <PATH>' \
             '--surfnet-id <ID>'",
        );
        check(command, Duration::from_secs(1)).unwrap();
    }

    #[test]
    fn refuses_old_and_unsuccessful_help_with_a_typed_error() {
        for body in [
            "printf '%s\\n' '--offline' '--afterwards'; exit 0",
            "printf '%s\\n' '--after' '--historical-config' '--snapshot' '--surfnet-id'; exit 1",
        ] {
            let failure = check(script(body), Duration::from_secs(1)).unwrap_err();
            assert!(
                failure.to_string().contains("SURFPOOL_INCOMPATIBLE"),
                "{failure:#}"
            );
        }
    }

    #[test]
    fn stops_and_reaps_a_hung_local_check() {
        let began = Instant::now();
        let failure = check(script("exec sleep 10"), Duration::from_millis(40)).unwrap_err();
        assert!(
            failure.to_string().contains("SURFPOOL_INCOMPATIBLE"),
            "{failure:#}"
        );
        assert!(began.elapsed() < Duration::from_secs(2));
    }
}
