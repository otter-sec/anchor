use {anchor_cli::Opts, anyhow::Result, clap::Parser, std::ffi::OsString};

fn main() -> Result<()> {
    // When used as a RUSTC_WRAPPER (set by `anchor debugger`), the process
    // is invoked as `anchor <real-rustc> <rustc-args...>` — not a normal
    // subcommand. Detect this early, before clap parsing, and delegate to
    // the wrapper logic that fixes DWARF source paths.
    #[cfg(not(windows))]
    if anchor_cli::debugger::rustc_wrapper::maybe_exec_as_wrapper() {
        // `maybe_exec_as_wrapper` calls `exec()` and never returns when
        // it detects wrapper mode. If it returns, we're in normal CLI mode.
        unreachable!();
    }

    if is_verbose_version_request() {
        print!("{}", anchor_cli::support_version_report());
        return Ok(());
    }

    anchor_cli::entry(Opts::parse())
}

fn is_verbose_version_request() -> bool {
    is_verbose_version_args(std::env::args_os().skip(1).collect())
}

fn is_verbose_version_args(args: Vec<OsString>) -> bool {
    match args.as_slice() {
        [arg] => arg == "-vV" || arg == "-Vv",
        [first, second] => {
            (is_version_arg(first) && is_verbose_arg(second))
                || (is_verbose_arg(first) && is_version_arg(second))
        }
        _ => false,
    }
}

fn is_version_arg(arg: &OsString) -> bool {
    arg == "--version" || arg == "-V"
}

fn is_verbose_arg(arg: &OsString) -> bool {
    arg == "--verbose" || arg == "-v"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn detects_verbose_version_requests() {
        for input in [
            &["-vV"][..],
            &["-Vv"],
            &["-v", "-V"],
            &["-V", "-v"],
            &["--verbose", "--version"],
            &["--version", "--verbose"],
        ] {
            assert!(is_verbose_version_args(args(input)));
        }
    }

    #[test]
    fn ignores_regular_version_requests() {
        for input in [&["-V"][..], &["--version"], &["version"], &[]] {
            assert!(!is_verbose_version_args(args(input)));
        }
    }
}
