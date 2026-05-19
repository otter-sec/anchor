use {anchor_cli::Opts, anyhow::Result, clap::Parser};

fn main() -> Result<()> {
    if anchor_cli::debugger::rustc_wrapper::maybe_exec_as_wrapper() {
        unreachable!();
    }

    anchor_cli::entry(Opts::parse())
}
