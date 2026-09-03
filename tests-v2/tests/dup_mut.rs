//! Integration tests for the duplicate-mutable-account safety check.
//!
//! Covers the common ways a caller might try to alias a mutable account:
//! two mut slots, three mut slots in every dup position, and a mut paired
//! with a read-only slot. Each should surface `Custom(2040)`
//! (`ErrorCode::ConstraintDuplicateMutableAccount`). The final test
//! exercises the `#[account(unsafe(dup))]` escape hatch; the on-chain
//! handler is written so even the aliased invocation never holds two
//! live `&mut Data` to the same bytes, so no UB.

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

/// Custom program error code for `ConstraintDuplicateMutableAccount`.
const DUPLICATE_MUT_ERROR: u32 = 2040;

fn program_id() -> Pubkey {
    "2TxMd2YAMi9Sk4xxiJBNkYQNuxK9FwvwwiujuEbKoanz"
        .parse()
        .unwrap()
}

fn data_pda(seed: u8) -> Pubkey {
    Pubkey::find_program_address(&[b"d", &[seed]], &program_id()).0
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
    let pda = data_pda(seed);
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

/// Build + send a transaction and return the raw litesvm result so
/// failure cases can inspect the on-chain error code.
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
fn assert_custom_error(result: &litesvm::types::TransactionResult, expected: u32) {
    let failure = match result {
        Ok(_) => panic!("expected transaction to fail with Custom({expected}), got success"),
        Err(f) => f,
    };
    let rendered = format!("{:?}", failure.err);
    assert!(
        rendered.contains(&format!("Custom({expected})")),
        "expected Custom({expected}), got: {rendered}",
    );
}

fn read_value(svm: &LiteSVM, pda: &Pubkey) -> u64 {
    let account = svm.get_account(pda).expect("account should exist");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

// ---------------------------------------------------------------------------
// Two-mut instruction
// ---------------------------------------------------------------------------

#[test]
fn test_two_mut_distinct_ok() {
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
fn test_two_mut_dup_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

// ---------------------------------------------------------------------------
// Three-mut instruction — cover every dup position (0,1), (0,2), (1,2)
// ---------------------------------------------------------------------------

#[test]
fn test_three_mut_all_distinct_ok() {
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
fn test_three_mut_dup_positions_0_and_1() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let c = init_data(&mut svm, &payer, 2);

    let data = dup_mut::instruction::TouchThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(a, false),
        AccountMeta::new(c, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_three_mut_dup_positions_0_and_2() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(a, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_three_mut_dup_positions_1_and_2() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(b, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

// ---------------------------------------------------------------------------
// Mut + readonly instruction — same pubkey still triggers the mut-side check
// (cursor marks both bits; the mut field's generated check fires).
// ---------------------------------------------------------------------------

#[test]
fn test_mut_readonly_distinct_ok() {
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
}

#[test]
fn test_mut_readonly_dup_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchMutAndReadonly { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

// ---------------------------------------------------------------------------
// Asymmetric unsafe(dup): only the second field opts out. The first field's
// generated check still fires on the aliased call, so the check is rejected.
// ---------------------------------------------------------------------------

#[test]
fn test_asym_unsafe_dup_still_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchTwoMutAsymUnsafe { value: 5 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_asym_unsafe_dup_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchTwoMutAsymUnsafe { value: 5 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("distinct pubkeys should succeed under asym unsafe(dup)");

    assert_eq!(read_value(&svm, &a), 5);
    assert_eq!(read_value(&svm, &b), 6);
}

// ---------------------------------------------------------------------------
// Symmetric unsafe(dup) on both fields: the duplicate check is skipped on
// every relevant position, so an aliased call is accepted. The handler is
// written to never hold two `&mut Data` live at once, avoiding UB.
// ---------------------------------------------------------------------------

#[test]
fn test_unsafe_dup_aliased_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchTwoMutUnsafe { value: 99 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("unsafe(dup) should allow same pubkey in both slots");

    // The handler writes via data_a only; data_b is never deref'd.
    assert_eq!(read_value(&svm, &a), 99);
}

#[test]
fn test_unsafe_dup_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchTwoMutUnsafe { value: 99 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("unsafe(dup) with distinct pubkeys should succeed");

    assert_eq!(read_value(&svm, &a), 99);
    // data_b is never written by the handler.
    assert_eq!(read_value(&svm, &b), 0);
}

// ===========================================================================
// Nested<Inner> variants — mirror the above cases one-for-one through a
// `Nested<Inner>` wrapper. This exercises the derive's `base_offset`
// threading: `Inner::try_accounts` is called with `__base_offset + offset`
// so bitvec indices stay in the global coordinate system.
// ===========================================================================

// -- Two mut via Nested -----------------------------------------------------

#[test]
fn test_nested_two_mut_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("nested distinct pubkeys should succeed");

    assert_eq!(read_value(&svm, &a), 7);
    assert_eq!(read_value(&svm, &b), 8);
}

#[test]
fn test_nested_two_mut_dup_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchNestedTwoMut { value: 7 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

// -- Three mut via Nested — every dup position -----------------------------

#[test]
fn test_nested_three_mut_all_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);
    let c = init_data(&mut svm, &payer, 2);

    let data = dup_mut::instruction::TouchNestedThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(c, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("nested all-distinct should succeed");

    assert_eq!(read_value(&svm, &a), 10);
    assert_eq!(read_value(&svm, &b), 11);
    assert_eq!(read_value(&svm, &c), 12);
}

#[test]
fn test_nested_three_mut_dup_positions_0_and_1() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let c = init_data(&mut svm, &payer, 2);

    let data = dup_mut::instruction::TouchNestedThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(a, false),
        AccountMeta::new(c, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_nested_three_mut_dup_positions_0_and_2() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(a, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_nested_three_mut_dup_positions_1_and_2() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedThreeMut { value: 10 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(b, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

// -- Mut + readonly via Nested ---------------------------------------------

#[test]
fn test_nested_mut_readonly_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedMutReadonly { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(b, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("nested distinct mut+readonly should succeed");

    assert_eq!(read_value(&svm, &a), 42);
}

#[test]
fn test_nested_mut_readonly_dup_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchNestedMutReadonly { value: 42 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new_readonly(a, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

// -- Asymmetric unsafe(dup) via Nested -------------------------------------

#[test]
fn test_nested_asym_unsafe_dup_still_rejected() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchNestedAsymUnsafe { value: 5 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_nested_asym_unsafe_dup_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedAsymUnsafe { value: 5 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("nested distinct under asym unsafe(dup)");

    assert_eq!(read_value(&svm, &a), 5);
    assert_eq!(read_value(&svm, &b), 6);
}

// -- Symmetric unsafe(dup) via Nested --------------------------------------

#[test]
fn test_nested_unsafe_dup_aliased_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);

    let data = dup_mut::instruction::TouchNestedUnsafe { value: 99 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(a, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("nested unsafe(dup) should allow same pubkey");

    assert_eq!(read_value(&svm, &a), 99);
}

#[test]
fn test_nested_unsafe_dup_distinct_ok() {
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchNestedUnsafe { value: 99 }.data();
    let metas = vec![AccountMeta::new(a, false), AccountMeta::new(b, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("nested unsafe(dup) with distinct pubkeys");

    assert_eq!(read_value(&svm, &a), 99);
    // data_b never written by the handler.
    assert_eq!(read_value(&svm, &b), 0);
}

// -- Cross-boundary: outer mut + Nested<InnerTwoMut> -----------------------
//
// Global offsets: outer=0, pair.data_a=1, pair.data_b=2.
// Exercises that bit indices stay global across the boundary — the
// duplicate-check constraint on the OUTER field and the constraint inside
// the inner struct both look at the same bitvec but with different
// `base_offset`s.

#[test]
fn test_outer_plus_nested_all_distinct_ok() {
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
        .expect("outer+nested all-distinct should succeed");

    assert_eq!(read_value(&svm, &a), 20);
    assert_eq!(read_value(&svm, &b), 21);
    assert_eq!(read_value(&svm, &c), 22);
}

#[test]
fn test_outer_dups_inner_first() {
    // outer (pos 0) aliases pair.data_a (pos 1).
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let c = init_data(&mut svm, &payer, 2);

    let data = dup_mut::instruction::TouchOuterMutPlusNested { value: 20 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(a, false),
        AccountMeta::new(c, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_outer_dups_inner_second() {
    // outer (pos 0) aliases pair.data_b (pos 2).
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchOuterMutPlusNested { value: 20 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(a, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn test_outer_plus_nested_inner_dup() {
    // Dup stays inside the nested struct — inner try_accounts catches it.
    let (mut svm, payer) = setup();
    let a = init_data(&mut svm, &payer, 0);
    let b = init_data(&mut svm, &payer, 1);

    let data = dup_mut::instruction::TouchOuterMutPlusNested { value: 20 }.data();
    let metas = vec![
        AccountMeta::new(a, false),
        AccountMeta::new(b, false),
        AccountMeta::new(b, false),
    ];
    let result = send_raw(&mut svm, data, metas, &payer);
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}
