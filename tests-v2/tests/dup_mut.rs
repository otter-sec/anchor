//! Integration tests for aliased account inputs.
//!
//! Data-carrying wrappers reject a conflicting borrow of an aliased account
//! with `AccountBorrowFailed`, in either field order and across `Nested`,
//! `Box`, and `Option` fields. Wrappers without typed data accept aliases,
//! and raw borrows or CPI handles through them still fail while a data
//! wrapper holds a conflicting borrow.

use {
    anchor_lang::{
        solana_program::instruction::{AccountMeta, Instruction},
        InstructionData,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "2TxMd2YAMi9Sk4xxiJBNkYQNuxK9FwvwwiujuEbKoanz"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    let deploy_str = deploy_dir.to_str().unwrap();

    build_program(
        test_dir.join("programs/dup-mut").to_str().unwrap(),
        deploy_str,
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), &deploy_dir.join("dup_mut.so"))
        .expect("failed to load dup-mut program");

    let payer = keypair_for("dup-mut-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn init_data(svm: &mut LiteSVM, payer: &Keypair, seed: u8) -> Pubkey {
    let pda = Pubkey::find_program_address(&[b"d", &[seed]], &program_id()).0;
    let data = dup_mut::instruction::Initialize { seed }.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[])
        .expect("initialize should succeed");
    pda
}

fn init_borsh(svm: &mut LiteSVM, payer: &Keypair, seed: u8) -> Pubkey {
    let pda = Pubkey::find_program_address(&[b"b", &[seed]], &program_id()).0;
    let data = dup_mut::instruction::InitializeBorsh { seed }.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[])
        .expect("initialize_borsh should succeed");
    pda
}

fn send_raw(
    svm: &mut LiteSVM,
    data: Vec<u8>,
    metas: Vec<AccountMeta>,
    payer: &Keypair,
) -> litesvm::types::TransactionResult {
    let ix = Instruction::new_with_bytes(program_id(), &data, metas);
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &signers).unwrap();
    svm.send_transaction(tx)
}

#[track_caller]
fn assert_borrow_failed(result: &litesvm::types::TransactionResult) {
    let failure = match result {
        Ok(_) => panic!("expected transaction to fail with AccountBorrowFailed, got success"),
        Err(f) => f,
    };
    let rendered = format!("{:?}", failure.err);
    assert!(
        rendered.contains("AccountBorrowFailed"),
        "expected AccountBorrowFailed, got: {rendered}",
    );
}

fn read_value(svm: &LiteSVM, pda: &Pubkey) -> u64 {
    let account = svm.get_account(pda).expect("account should exist");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

fn system_program() -> AccountMeta {
    AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false)
}

// ---------------------------------------------------------------------------
// Account<T> fields
// ---------------------------------------------------------------------------

#[test]
fn two_mut_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct pubkeys should succeed");

    assert_eq!(read_value(&svm, &a), 7);
    assert_eq!(read_value(&svm, &b), 8);
}

#[test]
fn two_mut_alias_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
}

#[test]
fn three_mut_alias_rejected_in_every_position() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    for keys in [[a, a, b], [a, b, a], [b, a, a]] {
        let data = dup_mut::instruction::TouchThreeMut { value: 10 }.data();
        let metas = keys.iter().map(|k| AccountMeta::new(*k, false)).collect();
        assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
    }
}

#[test]
fn three_mut_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);
    let c = init_data(&mut svm, &payer, 2);

    let data = dup_mut::instruction::TouchThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(c, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("all-distinct should succeed");

    assert_eq!(read_value(&svm, &a), 10);
    assert_eq!(read_value(&svm, &b), 11);
    assert_eq!(read_value(&svm, &c), 12);
}

#[test]
fn mut_then_readonly_alias_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchMutAndReadonly { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
}

#[test]
fn readonly_then_mut_alias_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchReadonlyAndMut { value: 42 }.data();
    let metas = vec![
        AccountMeta::new_readonly(a, false),
        AccountMeta::new(a, false),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
}

#[test]
fn mut_and_readonly_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchMutAndReadonly { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(b, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct mut+readonly should succeed");
    assert_eq!(read_value(&svm, &a), 42);

    let data = dup_mut::instruction::TouchReadonlyAndMut { value: 5 }.data();
    let metas = vec![
        AccountMeta::new_readonly(a, false),
        AccountMeta::new(b, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct readonly+mut should succeed");
    assert_eq!(read_value(&svm, &b), 47);
}

#[test]
fn two_readonly_alias_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::ReadTwo { expected: 0 }.data();
    let metas = vec![
        AccountMeta::new_readonly(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("two read-only loads may share an account");
}

// ---------------------------------------------------------------------------
// BorshAccount, Box, and Option fields
// ---------------------------------------------------------------------------

#[test]
fn borsh_mut_and_readonly() {
    let (mut svm, payer) = setup();
    let a = init_borsh(&mut svm, &payer, 0);
    let b = init_borsh(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchBorshMutAndReadonly { value: 3 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));

    let data = dup_mut::instruction::TouchBorshMutAndReadonly { value: 3 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(b, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct Borsh accounts should succeed");
    assert_eq!(read_value(&svm, &a), 3);
}

#[test]
fn boxed_mut_and_readonly() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchBoxedMutAndReadonly { value: 4 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));

    let data = dup_mut::instruction::TouchBoxedMutAndReadonly { value: 4 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(b, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct boxed + plain accounts should succeed");
    assert_eq!(read_value(&svm, &a), 4);
}

#[test]
fn optional_some_alias_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchOptionalMutAndMut { value: 9 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
}

#[test]
fn optional_none_holds_no_borrow() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchOptionalMutAndMut { value: 9 }.data();
    let metas = vec![
        AccountMeta::new_readonly(program_id(), false),
        AccountMeta::new(a, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("a None optional account should not conflict");
    assert_eq!(read_value(&svm, &a), 10);
}

// ---------------------------------------------------------------------------
// Nested<T> fields
// ---------------------------------------------------------------------------

#[test]
fn nested_two_mut() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));

    let data = dup_mut::instruction::TouchNestedTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct nested pubkeys should succeed");
    assert_eq!(read_value(&svm, &a), 7);
    assert_eq!(read_value(&svm, &b), 8);
}

#[test]
fn nested_mut_readonly_alias_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchNestedMutReadonly { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
}

#[test]
fn outer_and_nested_alias_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    for keys in [[a, a, b], [a, b, a]] {
        let data = dup_mut::instruction::TouchOuterMutPlusNested { value: 20 }.data();
        let metas = keys.iter().map(|k| AccountMeta::new(*k, false)).collect();
        assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
    }
}

#[test]
fn outer_and_nested_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);
    let c = init_data(&mut svm, &payer, 2);

    let data = dup_mut::instruction::TouchOuterMutPlusNested { value: 20 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(c, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct outer + nested pubkeys should succeed");
    assert_eq!(read_value(&svm, &a), 20);
    assert_eq!(read_value(&svm, &b), 21);
    assert_eq!(read_value(&svm, &c), 22);
}

// ---------------------------------------------------------------------------
// Wrappers without typed data
// ---------------------------------------------------------------------------

#[test]
fn signer_roles_accept_one_wallet() {
    let (mut svm, payer) = setup();

    let data = dup_mut::instruction::SignerRoles {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(payer.pubkey(), true),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("one wallet may be both payer and authority");
}

#[test]
fn system_transfer_accepts_payer_as_recipient() {
    let (mut svm, payer) = setup();

    let data = dup_mut::instruction::TransferToRecipient { lamports: 1_000 }.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(payer.pubkey(), false),
        system_program(),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("one wallet may be both payer and recipient of a system transfer");
}

#[test]
fn system_transfer_to_distinct_recipient() {
    let (mut svm, payer) = setup();
    let recipient = Pubkey::new_unique();

    let data = dup_mut::instruction::TransferToRecipient { lamports: 1_000_000 }.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(recipient, false),
        system_program(),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("transfer to a distinct recipient should succeed");
    assert_eq!(svm.get_account(&recipient).unwrap().lamports, 1_000_000);
}

#[test]
fn unchecked_alias_of_mut_data_loads_but_cannot_borrow() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchDataAndRaw {
        value: 5,
        borrow_raw: false,
    }
    .data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("an UncheckedAccount alias holds no borrow");
    assert_eq!(read_value(&svm, &a), 5);

    let data = dup_mut::instruction::TouchDataAndRaw {
        value: 6,
        borrow_raw: true,
    }
    .data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));
}

#[test]
fn cpi_through_unchecked_alias_of_mut_data_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TransferToRawAlias { value: 8 }.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(a, false),
        AccountMeta::new(a, false),
        system_program(),
    ];
    assert_borrow_failed(&send_raw(&mut svm, data, metas, &payer));

    let data = dup_mut::instruction::TransferToRawAlias { value: 8 }.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        system_program(),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("CPI through a distinct unchecked account should succeed");
    assert_eq!(read_value(&svm, &a), 8);
}
