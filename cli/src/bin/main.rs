use {anchor_cli::Opts, anyhow::Result, clap::Parser, std::ffi::OsString};

fn main() -> Result<()> {
    #[cfg(not(windows))]
    if anchor_cli::debugger::rustc_wrapper::maybe_exec_as_wrapper() {
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
