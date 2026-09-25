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
    clap::{Parser, Subcommand},
};

#[derive(Parser)]
#[command(about = "Run and maintain Anchor account deserialization benchmarks")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run benchmarks for one or more Anchor versions.
    Bench {
        /// Recorded Anchor versions, `unreleased` (the default), or `all`.
        versions: Vec<String>,
        /// Print commands and output from AVM, Cargo, and toolchain installers.
        #[arg(short, long)]
        verbose: bool,
        /// Do not update results and fail if a measurement changes by more than 1%.
        #[arg(long)]
        check: bool,
    },
    /// Move the unreleased benchmark baseline to a new stable release.
    BumpVersion {
        /// Stable Anchor version being released.
        version: String,
    },
}

fn main() -> Result<()> {
    match Args::parse().command {
        Command::Bench {
            versions,
            verbose,
            check,
        } => bench(versions, verbose, check),
        Command::BumpVersion { version } => Runner::new(false)?.bump_version(&version),
    }
}

fn bench(requested: Vec<String>, verbose: bool, check: bool) -> Result<()> {
    let mut runner = Runner::new(verbose)?;
    let versions = runner.select_versions(&requested)?;

    runner.build_avm()?;
    let previous_solana = runner.active_solana();
    let result = runner.run_benchmarks(&versions, check);
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
