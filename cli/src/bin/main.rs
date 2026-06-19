use {anchor_cli::Opts, anyhow::Result, clap::Parser, std::ffi::OsString, std::path::PathBuf};

fn find_system_binary(binary_name: &str) -> Option<PathBuf> {
    let target_name = if cfg!(windows) {
        if binary_name.ends_with(".exe") {
            binary_name.to_string()
        } else {
            format!("{binary_name}.exe")
        }
    } else {
        binary_name.to_string()
    };

    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let candidate = path.join(&target_name);
            if candidate.is_file() {
                let path_str = candidate.to_string_lossy().to_lowercase();
                if path_str.contains("solana") || path_str.contains("platform-tools") {
                    continue;
                }
                return Some(candidate);
            }
        }
    }
    None
}

fn main() -> Result<()> {
    if std::env::var("ANCHOR_USE_SYSTEM_CARGO").map(|v| v == "true").unwrap_or(false) {
        if let Some(cargo_path) = find_system_binary("cargo") {
            std::env::set_var("CARGO", cargo_path);
        }
    }
    if std::env::var("ANCHOR_USE_SYSTEM_RUST").map(|v| v == "true").unwrap_or(false) {
        if let Some(rustc_path) = find_system_binary("rustc") {
            std::env::set_var("RUSTC", rustc_path);
        }
    }

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
