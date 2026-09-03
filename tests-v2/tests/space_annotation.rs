use {
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for},
};

fn program_id() -> Pubkey {
    "Spac111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/space-annotation").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("space_annotation.so"))
        .expect("load space_annotation program");
    let payer = keypair_for("space-annotation-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn pda(seed: &[u8]) -> Pubkey {
    Pubkey::find_program_address(&[seed], &program_id()).0
}

fn send(
    svm: &mut LiteSVM,
    discrim: u8,
    seed: &[u8],
    payer: &Keypair,
) -> (Pubkey, TransactionResult) {
    let address = pda(seed);
    let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        program_id(),
        &[discrim],
        vec![
            anchor_lang::solana_program::instruction::AccountMeta::new(payer.pubkey(), true),
            anchor_lang::solana_program::instruction::AccountMeta::new(address, false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                solana_sdk_ids::system_program::ID,
                false,
            ),
        ],
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    (address, svm.send_transaction(tx))
}

fn assert_failed_without_account(svm: &mut LiteSVM, discrim: u8, seed: &[u8], payer: &Keypair) {
    let (address, result) = send(svm, discrim, seed, payer);
    assert!(
        result.is_err(),
        "space annotation should reject underallocated account"
    );
    assert!(
        svm.get_account(&address).is_none(),
        "failed init should roll back created account"
    );
}

fn assert_initialized_with_value(
    svm: &mut LiteSVM,
    discrim: u8,
    seed: &[u8],
    payer: &Keypair,
    expected_space: usize,
    expected_value: u64,
) {
    let (address, result) = send(svm, discrim, seed, payer);
    result.expect("space annotation init should succeed");
    let account = svm.get_account(&address).expect("account exists");
    assert_eq!(account.data.len(), expected_space);
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    assert_eq!(value, expected_value);
}

#[test]
fn pod_account_explicit_space_rejects_underallocation_and_accepts_overallocation() {
    let (mut svm, payer) = setup();

    assert_failed_without_account(&mut svm, 0, b"pod-disc", &payer);
    assert_failed_without_account(&mut svm, 1, b"pod-under", &payer);
    assert_initialized_with_value(&mut svm, 2, b"pod-exact", &payer, 16, 16);
    assert_initialized_with_value(&mut svm, 3, b"pod-over", &payer, 32, 32);
}

#[test]
fn borsh_account_explicit_space_rejects_underallocation_and_accepts_overallocation() {
    let (mut svm, payer) = setup();

    assert_failed_without_account(&mut svm, 4, b"borsh-disc", &payer);
    assert_failed_without_account(&mut svm, 5, b"borsh-payload", &payer);
    assert_initialized_with_value(&mut svm, 6, b"borsh-exact", &payer, 16, 16);
    assert_initialized_with_value(&mut svm, 7, b"borsh-over", &payer, 24, 24);
}

#[test]
fn tail_slab_explicit_space_rejects_underallocation_and_accepts_capacity_space() {
    let (mut svm, payer) = setup();

    assert_failed_without_account(&mut svm, 8, b"tail-under", &payer);
    assert_initialized_with_value(&mut svm, 9, b"tail-exact", &payer, 32, 32);

    let (address, result) = send(&mut svm, 10, b"tail-over", &payer);
    result.expect("overallocated tail slab init should succeed");
    let account = svm.get_account(&address).expect("tail slab exists");
    assert_eq!(account.data.len(), 56);
    let value = u64::from_le_bytes(account.data[8..16].try_into().unwrap());
    let space_seen = u64::from_le_bytes(account.data[16..24].try_into().unwrap());
    let len = u32::from_le_bytes(account.data[24..28].try_into().unwrap());
    assert_eq!(value, 56);
    assert_eq!(space_seen, 56);
    assert_eq!(len, 0);
}
