mod cases;
mod comparison;
mod fixture;
mod litesvm;
mod markdown;
mod results;
mod runner;
mod stack;
mod toolchain;
mod version;

use {
    crate::runner::Runner,
    anyhow::{bail, Result},
    clap::Parser,
};

#[derive(Parser)]
#[command(about = "Benchmark Anchor account deserialization across releases")]
struct Args {
    /// Recorded Anchor versions, `unreleased` (the default), or `all`.
    versions: Vec<String>,
    /// Print commands and output from AVM, Cargo, and toolchain installers.
    #[arg(short, long)]
    verbose: bool,
    /// Do not update results and fail if a measurement changes by more than 1%.
    #[arg(long)]
    check: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut runner = Runner::new(args.verbose)?;
    let versions = runner.select_versions(&args.versions)?;

    runner.build_avm()?;
    let previous_solana = runner.active_solana();
    let result = runner.run_benchmarks(&versions, args.check);
    let restore = runner.restore_solana(previous_solana.as_deref());
    match (result, restore) {
        (Err(error), Err(restore)) => {
            Err(error.context(format!("Also failed to restore Solana: {restore:#}")))
        }
        (Err(error), _) => Err(error),
        (_, Err(error)) => Err(error),
        (Ok(true), Ok(())) => bail!("Benchmark change exceeded the 1% threshold"),
        (Ok(false), Ok(())) => Ok(()),
    }
}
