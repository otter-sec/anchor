use {anchor_lang::prelude::*, anyhow::Result, clap::Parser, solana_sdk::pubkey::Pubkey};

anchor_lang::declare_program!(basic_2);
anchor_lang::declare_program!(basic_4);
anchor_lang::declare_program!(composite);
anchor_lang::declare_program!(events);
anchor_lang::declare_program!(optional);

// Associated constants and PDA seed literals are not included in the IDL.
const DATA_ACCOUNT_SPACE: usize = 8 + 8; // Discriminator followed by a u64.
const DATA_PDA_SEED: &[u8] = b"data_pda";

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

#[cfg(test)]
mod tests;
