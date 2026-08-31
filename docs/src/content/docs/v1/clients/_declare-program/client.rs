use anchor_client::{
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        commitment_config::CommitmentConfig, native_token::LAMPORTS_PER_SOL, signature::Keypair,
        system_program,
    },
    solana_signer::Signer,
    Client, Cluster,
};
use anchor_lang::prelude::*;
use std::rc::Rc;

declare_program!(example);
use example::{
    accounts::Counter, // Program account types
    client::accounts,  // Accounts for program instructions
    client::args,      // Arguments for program instructions
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let connection = RpcClient::new_with_commitment(
        "http://127.0.0.1:8899",
        CommitmentConfig::confirmed(),
    );

    let payer = Keypair::new();
    let counter = Keypair::new();
    println!("Generated Keypairs:");
    println!("   Payer: {}", payer.pubkey());
    println!("   Counter: {}", counter.pubkey());

    println!("\nRequesting 1 SOL airdrop to payer");
    let airdrop_signature = connection.request_airdrop(&payer.pubkey(), LAMPORTS_PER_SOL)?;

    while !connection.confirm_transaction(&airdrop_signature)? {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    println!("   Airdrop confirmed!");

    let provider = Client::new_with_options(
        Cluster::Localnet,
        Rc::new(payer),
        CommitmentConfig::confirmed(),
    );
    let program = provider.program(example::ID)?;

    println!("\nSend transaction with initialize and increment instructions");
    let initialize_ix = program
        .request()
        .accounts(accounts::Initialize {
            counter: counter.pubkey(),
            payer: program.payer(),
            system_program: system_program::ID,
        })
        .args(args::Initialize)
        .instructions()?
        .remove(0);

    let increment_ix = program
        .request()
        .accounts(accounts::Increment {
            counter: counter.pubkey(),
        })
        .args(args::Increment)
        .instructions()?
        .remove(0);

    let signature = program
        .request()
        .instruction(initialize_ix)
        .instruction(increment_ix)
        .signer(&counter)
        .send()
        .await?;
    println!("   Transaction confirmed: {}", signature);

    println!("\nFetch counter account data");
    let counter_account: Counter = program.account::<Counter>(counter.pubkey()).await?;
    println!("   Counter value: {}", counter_account.count);
    Ok(())
}
