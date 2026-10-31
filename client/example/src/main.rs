use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

#[cfg(not(feature = "async"))]
mod blocking;

#[cfg(feature = "async")]
mod nonblocking;

#[derive(Parser, Debug)]
pub struct Opts {
    #[clap(long)]
    composite_pid: Pubkey,
    #[clap(long)]
    basic_2_pid: Pubkey,
    #[clap(long)]
    basic_4_pid: Pubkey,
    #[clap(long)]
    events_pid: Pubkey,
    #[clap(long)]
    events_caller_pid: Pubkey,
    #[clap(long)]
    optional_pid: Pubkey,
    #[clap(long, default_value = "false")]
    multithreaded: bool,
}

// This example assumes a local validator is running with the programs
// deployed at the addresses given by the CLI args.
#[cfg(not(feature = "async"))]
fn main() -> Result<()> {
    blocking::main()
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<()> {
    nonblocking::main().await
}
