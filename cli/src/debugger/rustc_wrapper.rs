//! RUSTC_WRAPPER shim that restores absolute paths in SBF DWARF output.
//!
//! ## Problem
//!
//! The Solana toolchain's cargo passes `-Zremap-cwd-prefix=` (empty
//! replacement) to rustc for every SBF crate. This strips `DW_AT_comp_dir`
//! from the DWARF, making all source paths relative. When multiple crates
//! share filenames like `src/lib.rs`, the debugger can't tell them apart
//! and may show source from the wrong crate.
//!
//! ## Solution
//!
//! `anchor debugger` sets `RUSTC_WRAPPER` to the `anchor` binary itself.
//! Cargo then invokes `anchor <real-rustc> <args...>` for every rustc
//! call. This module detects that invocation pattern (the env var
//! `__ANCHOR_RUSTC_WRAPPER=1` disambiguates from normal CLI usage) and
//! replaces `-Zremap-cwd-prefix=` with `-Zremap-cwd-prefix=$CWD`,
//! preserving absolute paths in the debug info.
//!
//! The sentinel env var is necessary because `RUSTC_WRAPPER` mode passes
//! a path as argv[1] (the real rustc binary), which clap would reject as
//! an unknown subcommand. The check in `main.rs` runs before clap
//! parsing so the process never hits the normal CLI dispatch.
//!
//! ## Performance
//!
//! The wrapper adds ~1ms of fork+exec overhead per rustc invocation.
//! This is negligible compared to actual compilation time.

use std::{
    ffi::OsString,
    fs,
    path::Path,
    process,
};

/// Env var set by `anchor debugger` before calling `cargo build-sbf`.
/// When present, the process knows it was invoked as a RUSTC_WRAPPER
/// and should rewrite args instead of running the normal CLI.
pub const WRAPPER_SENTINEL: &str = "__ANCHOR_RUSTC_WRAPPER";

fn rewrite_rustc_args(args: &[OsString], cwd: &Path) -> Vec<OsString> {
    args.iter()
        .flat_map(|arg| {
            let Some(path) = arg.to_str().and_then(|arg| arg.strip_prefix('@')) else {
                return vec![arg.clone()];
            };

            match fs::read_to_string(path) {
                Ok(contents) => contents
                    .lines()
                    .map(|line| {
                        let arg = line.trim();
                        if arg == "-Zremap-cwd-prefix=" {
                            format!("-Zremap-cwd-prefix={}", cwd.display()).into()
                        } else {
                            arg.into()
                        }
                    })
                    .collect(),
                Err(_) => vec![arg.clone()],
            }
        })
        .collect()
}

/// If we're running as a RUSTC_WRAPPER (sentinel env var is set),
/// rewrite the rustc args and exec the real compiler. Never returns.
///
/// If we're NOT in wrapper mode, returns `false` so the caller can
/// proceed with normal CLI parsing.
pub fn maybe_exec_as_wrapper() -> bool {
    if std::env::var_os(WRAPPER_SENTINEL).is_none() {
        return false;
    }

    let args: Vec<OsString> = std::env::args_os().collect();
    // RUSTC_WRAPPER invocation: argv[0]=anchor, argv[1]=rustc, argv[2..]=args
    if args.len() < 2 {
        eprintln!("anchor rustc-wrapper: expected <rustc> <args...>");
        process::exit(1);
    }

    let rustc = &args[1];
    let cwd = std::env::current_dir().unwrap_or_default();
    let rewritten = rewrite_rustc_args(&args[2..], &cwd);

    let status = process::Command::new(rustc)
        .args(rewritten)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("anchor rustc-wrapper: failed to exec {rustc}: {e}");
            process::exit(1);
        });

    process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use {super::*, tempfile::tempdir};

    #[test]
    fn rewrites_direct_rustc_args() {
        let cwd = Path::new("/workspace/project");
        let args = [
            OsString::from("--crate-name"),
            OsString::from("project"),
            OsString::from("-Zremap-cwd-prefix="),
            OsString::from("src/lib.rs"),
        ];

        assert_eq!(
            rewrite_rustc_args(&args, cwd),
            [
                OsString::from("--crate-name"),
                OsString::from("project"),
                OsString::from("-Zremap-cwd-prefix=/workspace/project"),
                OsString::from("src/lib.rs"),
            ]
        );
    }

    #[test]
    fn expands_and_rewrites_rustc_argfile() {
        let temp = tempdir().unwrap();
        let argfile_dir = temp.path().join("path with spaces");
        fs::create_dir(&argfile_dir).unwrap();
        let argfile = argfile_dir.join("rustc args");
        fs::write(
            &argfile,
            "--crate-name\nproject\n\n-Zremap-cwd-prefix=\nsrc/lib.rs\n",
        )
        .unwrap();

        let args = [OsString::from(format!("@{}", argfile.display()))];
        assert_eq!(
            rewrite_rustc_args(&args, Path::new("/workspace/project")),
            [
                OsString::from("--crate-name"),
                OsString::from("project"),
                OsString::from(""),
                OsString::from("-Zremap-cwd-prefix=/workspace/project"),
                OsString::from("src/lib.rs"),
            ]
        );
    }

    #[test]
    fn preserves_unreadable_argfile_reference() {
        let args = [OsString::from("@path that does not exist")];

        assert_eq!(
            rewrite_rustc_args(&args, Path::new("/workspace/project")),
            args
        );
    }
}
