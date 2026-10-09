//! Smoke test for `anchor debugger`: load the .so, run deposit + withdraw.

use {
    anchor_v2_testing::{
        litesvm::LiteSVM, Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    solana_account::Account as SolanaAccount,
    solana_instruction::{AccountMeta, Instruction},
    solana_pubkey::Pubkey,
};

const PROGRAM_ID: &str = "33333333333333333333333333333333333333333333";
const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0u8; 32]);

fn setup() -> (LiteSVM, Pubkey, Keypair) {
    let program_id: Pubkey = PROGRAM_ID.parse().unwrap();
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../../target/deploy/vault_quasar.so");
    svm.add_program(program_id, bytes).unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    (svm, program_id, user)
}

fn make_ix(disc: u8, amount: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(9);
    data.push(disc);
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

#[test]
fn test_deposit() {
    let (mut svm, program_id, user) = setup();
    let (vault, _) = Pubkey::find_program_address(&[b"vault", user.pubkey().as_ref()], &program_id);

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: make_ix(0, 1_000_000),
    };
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    svm.send_transaction(tx).expect("deposit");
}

#[test]
fn test_withdraw() {
    let (mut svm, program_id, user) = setup();
    let (vault, _) = Pubkey::find_program_address(&[b"vault", user.pubkey().as_ref()], &program_id);

    svm.set_account(
        vault,
        SolanaAccount {
            lamports: 1_000_000_000,
            data: vec![],
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(vault, false),
        ],
        data: make_ix(1, 500_000),
    };
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&user.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&user]).unwrap();
    svm.send_transaction(tx).expect("withdraw");
}
