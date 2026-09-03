use {
    anchor_v2_testing::{Keypair, LiteSVM, Message, Signer, VersionedMessage, VersionedTransaction},
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata},
    solana_account::Account as SolanaAccount,
    vault_v2::instruction,
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/vault_v2.so");
    svm.add_program(vault_v2::id(), bytes).unwrap();

    let user = Keypair::new();
    svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    (svm, user)
}

fn send(svm: &mut LiteSVM, ix: anchor_lang::solana_program::instruction::Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn vault_address(user: &anchor_lang::Address) -> (anchor_lang::Address, u8) {
    vault_v2::accounts::Deposit::find_vault_address(user)
}

#[test]
fn test_deposit() {
    let (mut svm, user) = setup();
    let (vault, _) = vault_address(&user.pubkey());

    let ix = instruction::Deposit { amount: 1_000_000 }.to_instruction(
        vault_v2::accounts::DepositResolved { user: user.pubkey() },
    );
    let res = send(&mut svm, ix, &[&user]).expect("deposit");
    println!("deposit CUs: {}", res.compute_units_consumed);

    let vault_account = svm.get_account(&vault).expect("vault account");
    assert!(vault_account.lamports >= 1_000_000);
}

#[test]
fn test_withdraw() {
    let (mut svm, user) = setup();
    let (vault, _) = vault_address(&user.pubkey());

    // Withdraw uses direct lamport arithmetic, which requires the vault to
    // be program-owned. System transfer leaves it system-owned, so we
    // pre-fund it as a program-owned account (same as the bench harness).
    svm.set_account(
        vault,
        SolanaAccount {
            lamports: 1_000_000_000,
            data: vec![],
            owner: vault_v2::id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let ix = instruction::Withdraw { amount: 500_000 }.to_instruction(
        vault_v2::accounts::WithdrawResolved { user: user.pubkey() },
    );
    let res = send(&mut svm, ix, &[&user]).expect("withdraw");
    println!("withdraw CUs: {}", res.compute_units_consumed);

    let vault_account = svm.get_account(&vault).expect("vault account");
    assert_eq!(vault_account.lamports, 999_500_000);
}
