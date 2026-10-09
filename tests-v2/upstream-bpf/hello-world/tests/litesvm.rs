//! Runs the upstream-BPF build of `hello-world` under LiteSVM.
//!
//! Build the program first: `cargo build-bpf`.

use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

#[test]
fn hello() {
    let so = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/deploy/hello_world.so");
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(hello_world::ID, &so)
        .unwrap_or_else(|e| panic!("load {} (run `cargo build-bpf` first): {e:?}", so.display()));

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let ix = Instruction::new_with_bytes(hello_world::ID, &hello_world::instruction::Hello {}.data(), vec![]);
    let tx = Transaction::new(
        &[&payer],
        Message::new(&[ix], Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    let meta = svm.send_transaction(tx).expect("hello should succeed");

    println!("{}", meta.pretty_logs());
    assert!(meta.logs.iter().any(|l| l == "Program log: Hello, world!"));
}
