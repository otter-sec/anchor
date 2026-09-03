use {
    anchor_lang::{
        accounts::Account, bytemuck,
        solana_program::instruction::{AccountMeta, Instruction},
        InstructionData, Space, ToAccountMetas,
    },
    anchor_v2_testing::{Keypair, LiteSVM, Message, Signer, VersionedMessage, VersionedTransaction},
    litesvm::types::{FailedTransactionMetadata, TransactionMetadata},
    nested_v2::{instruction, Counter},
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/nested_v2.so");
    svm.add_program(nested_v2::id(), bytes).unwrap();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    (svm, admin)
}

fn send(svm: &mut LiteSVM, ix: Instruction, signers: &[&Keypair]) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn counter_address() -> anchor_lang::Address {
    nested_v2::accounts::Initialize::find_counter_address().0
}

fn config_address() -> anchor_lang::Address {
    nested_v2::accounts::Initialize::find_config_address().0
}

fn init(svm: &mut LiteSVM, admin: &Keypair) {
    let ix = instruction::Initialize {}.to_instruction(
        nested_v2::accounts::InitializeResolved { admin: admin.pubkey() },
    );
    send(svm, ix, &[admin]).expect("initialize");
}

fn read_counter(svm: &LiteSVM) -> u64 {
    let acct = svm.get_account(&counter_address()).expect("counter");
    let c: &Counter = bytemuck::from_bytes(&acct.data[8..]);
    c.value
}

#[test]
fn test_initialize() {
    let (mut svm, admin) = setup();
    init(&mut svm, &admin);

    assert_eq!(read_counter(&svm), 0);
}

#[test]
fn test_increment() {
    let (mut svm, admin) = setup();
    init(&mut svm, &admin);

    let config = config_address();
    let counter = counter_address();

    // Accounts: Nested<AdminConfig>{admin, config}, counter
    let metas = vec![
        AccountMeta::new_readonly(admin.pubkey(), true),
        AccountMeta::new_readonly(config, false),
        AccountMeta::new(counter, false),
    ];
    let ix = Instruction::new_with_bytes(
        nested_v2::id(),
        &instruction::Increment {}.data(),
        metas,
    );
    let res = send(&mut svm, ix, &[&admin]).expect("increment");
    println!("increment CUs: {}", res.compute_units_consumed);

    assert_eq!(read_counter(&svm), 1);
}

#[test]
fn test_reset() {
    let (mut svm, admin) = setup();
    init(&mut svm, &admin);

    let config = config_address();
    let counter = counter_address();

    // Increment first
    let metas = vec![
        AccountMeta::new_readonly(admin.pubkey(), true),
        AccountMeta::new_readonly(config, false),
        AccountMeta::new(counter, false),
    ];
    let ix = Instruction::new_with_bytes(
        nested_v2::id(),
        &instruction::Increment {}.data(),
        metas.clone(),
    );
    send(&mut svm, ix, &[&admin]).expect("increment");
    assert_eq!(read_counter(&svm), 1);

    // Reset
    let ix = Instruction::new_with_bytes(
        nested_v2::id(),
        &instruction::Reset {}.data(),
        metas,
    );
    let res = send(&mut svm, ix, &[&admin]).expect("reset");
    println!("reset CUs: {}", res.compute_units_consumed);

    assert_eq!(read_counter(&svm), 0);
}
