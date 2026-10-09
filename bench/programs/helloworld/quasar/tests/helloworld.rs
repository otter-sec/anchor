//! Smoke test for `anchor debugger`: load the .so, run `init`, assert success.

use {
    anchor_v2_testing::{
        litesvm::LiteSVM, Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::Pubkey,
};

const PROGRAM_ID: &str = "B7ihZyoXZ1fwAY3TugkiFJ6SXkzJwMuQrxrekBaSmn32";
const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0u8; 32]);

#[test]
fn test_init() {
    let program_id: Pubkey = PROGRAM_ID.parse().unwrap();
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../../target/deploy/hello_world_quasar.so");
    svm.add_program(program_id, bytes).unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    let (counter, _) = Pubkey::find_program_address(&[b"counter"], &program_id);

    // Quasar helloworld uses a 1-byte discriminator (init = 0).
    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(counter, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: vec![0u8],
    };
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx).expect("init");
    println!("init CUs: {}", res.compute_units_consumed);
}
