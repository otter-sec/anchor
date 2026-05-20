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

use std::process;

/// Env var set by `anchor debugger` before calling `cargo build-sbf`.
/// When present, the process knows it was invoked as a RUSTC_WRAPPER
/// and should rewrite args instead of running the normal CLI.
pub const WRAPPER_SENTINEL: &str = "__ANCHOR_RUSTC_WRAPPER";

/// If we're running as a RUSTC_WRAPPER (sentinel env var is set),
/// rewrite the rustc args and exec the real compiler. Never returns.
///
/// If we're NOT in wrapper mode, returns `false` so the caller can
/// proceed with normal CLI parsing.
pub fn maybe_exec_as_wrapper() -> bool {
    if std::env::var_os(WRAPPER_SENTINEL).is_none() {
        return false;
    }

    let args: Vec<String> = std::env::args().collect();
    // RUSTC_WRAPPER invocation: argv[0]=anchor, argv[1]=rustc, argv[2..]=args
    if args.len() < 2 {
        eprintln!("anchor rustc-wrapper: expected <rustc> <args...>");
        process::exit(1);
    }

    let rustc = &args[1];
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let rewritten: Vec<String> = args[2..]
        .iter()
        .map(|arg| {
            if arg == "-Zremap-cwd-prefix=" {
                format!("-Zremap-cwd-prefix={cwd}")
            } else {
                arg.clone()
            }
        })
        .collect();

    let status = process::Command::new(rustc)
        .args(&rewritten)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("anchor rustc-wrapper: failed to exec {rustc}: {e}");
            process::exit(1);
        });

    process::exit(status.code().unwrap_or(1));
}
