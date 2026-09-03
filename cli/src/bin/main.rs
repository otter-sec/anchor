use {anchor_cli::Opts, anyhow::Result, clap::Parser};

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

    anchor_cli::entry(Opts::parse())
}
