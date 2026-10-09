use {
    anchor_lang::{InstructionData, ToAccountMetas},
    anchor_v2_testing::{
        litesvm::{
            types::{FailedTransactionMetadata, TransactionMetadata},
            LiteSVM,
        },
        Keypair, Message, Signer, VersionedMessage, VersionedTransaction,
    },
    nested_v1::instruction,
    solana_pubkey::Pubkey,
};

type TxResult = Result<TransactionMetadata, FailedTransactionMetadata>;

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = anchor_v2_testing::svm();
    let bytes = include_bytes!("../../../../target/deploy/nested_v1.so");
    svm.add_program(nested_v1::id(), bytes).unwrap();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();

    (svm, admin)
}

fn send(
    svm: &mut LiteSVM,
    ix: anchor_lang::solana_program::instruction::Instruction,
    signers: &[&Keypair],
) -> TxResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&signers[0].pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn config_address() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"config"], &nested_v1::id())
}

fn counter_address() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"counter"], &nested_v1::id())
}

fn init(svm: &mut LiteSVM, admin: &Keypair) {
    let (config, _) = config_address();
    let (counter, _) = counter_address();

    let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        nested_v1::id(),
        &instruction::Initialize {}.data(),
        nested_v1::accounts::Initialize {
            admin: admin.pubkey(),
            config,
            counter,
            system_program: anchor_lang::solana_program::system_program::ID,
        }
        .to_account_metas(None),
    );
    send(svm, ix, &[admin]).expect("initialize");
}

#[test]
fn test_initialize() {
    let (mut svm, admin) = setup();
    init(&mut svm, &admin);
}

#[test]
fn test_increment() {
    let (mut svm, admin) = setup();
    init(&mut svm, &admin);

    let (config, _) = config_address();
    let (counter, _) = counter_address();

    let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        nested_v1::id(),
        &instruction::Increment {}.data(),
        nested_v1::accounts::Increment {
            admin: admin.pubkey(),
            config,
            counter,
        }
        .to_account_metas(None),
    );
    let res = send(&mut svm, ix, &[&admin]).expect("increment");
    println!("increment CUs: {}", res.compute_units_consumed);
}

#[test]
fn test_reset() {
    let (mut svm, admin) = setup();
    init(&mut svm, &admin);

    let (config, _) = config_address();
    let (counter, _) = counter_address();

    // Increment first
    let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        nested_v1::id(),
        &instruction::Increment {}.data(),
        nested_v1::accounts::Increment {
            admin: admin.pubkey(),
            config,
            counter,
        }
        .to_account_metas(None),
    );
    send(&mut svm, ix, &[&admin]).expect("increment");

    // Reset
    let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
        nested_v1::id(),
        &instruction::Reset {}.data(),
        nested_v1::accounts::Reset {
            admin: admin.pubkey(),
            config,
            counter,
        }
        .to_account_metas(None),
    );
    let res = send(&mut svm, ix, &[&admin]).expect("reset");
    println!("reset CUs: {}", res.compute_units_consumed);
}
