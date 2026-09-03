//! Integration tests for user-defined `AccountConstraint` impls.
//!
//! The `custom-constraints` test program defines four constraint markers
//! in `counter_ns`, each overriding exactly one of the four trait methods
//! (`init`, `check`, `update`, `exit`). These tests drive each handler
//! and inspect the persisted `Counter.value` to prove that the derive
//! routed each call to the correct method at the correct lifecycle
//! phase.

use {
    anchor_lang::{solana_program::instruction::AccountMeta, InstructionData},
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "CC111111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"counter"], &program_id()).0
}

fn boxed_counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"boxed-counter"], &program_id()).0
}

fn optional_counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"optional-counter"], &program_id()).0
}

fn optional_boxed_counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"optional-boxed-counter"], &program_id()).0
}

fn optional_init_if_needed_counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"optional-init-if-needed-counter"], &program_id()).0
}

fn fresh_zeroed_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"fresh-zeroed-counter"], &program_id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir
            .join("programs/custom-constraints")
            .to_str()
            .unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), &deploy_dir.join("custom_constraints.so"))
        .expect("load custom-constraints program");

    let payer = keypair_for("custom-constraints-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

/// Parse the Borsh-encoded `Counter.value` from an account. Layout:
/// `[disc: 8][value: 8 LE]`.
fn read_counter_value(svm: &LiteSVM, pda: &Pubkey) -> u64 {
    let account = svm.get_account(pda).expect("counter account exists");
    assert!(account.data.len() >= 16, "counter data too short");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

fn init_counter(svm: &mut LiteSVM, payer: &Keypair) {
    let data = custom_constraints::instruction::HandleInit {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[])
        .expect("handle_init should succeed");
}

fn init_boxed_counter(svm: &mut LiteSVM, payer: &Keypair) {
    let data = custom_constraints::instruction::HandleBoxedInit {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(boxed_counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[])
        .expect("handle_boxed_init should succeed");
}

fn init_optional_if_needed_counter(svm: &mut LiteSVM, payer: &Keypair) {
    let data = custom_constraints::instruction::HandleOptionalInitIfNeeded {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(optional_init_if_needed_counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[])
        .expect("handle_optional_init_if_needed should succeed");
}

fn set_zeroed_counter_account(svm: &mut LiteSVM, pda: Pubkey) {
    let account = solana_account::Account {
        lamports: 1_000_000,
        data: vec![0u8; 16],
        owner: program_id(),
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(pda, account).expect("set zeroed counter");
}

// ---- `init` method ---------------------------------------------------------

#[test]
fn init_hook_stamps_value_on_creation() {
    let (mut svm, payer) = setup();
    init_counter(&mut svm, &payer);

    // `InitValueConstraint::init` wrote 5 after BorshAccount created the
    // zero-filled account. A bare init without the constraint would leave
    // `value == 0`.
    assert_eq!(read_counter_value(&svm, &counter_pda()), 5);
}

#[test]
fn boxed_init_hook_stamps_value_on_creation() {
    let (mut svm, payer) = setup();
    init_boxed_counter(&mut svm, &payer);

    assert_eq!(read_counter_value(&svm, &boxed_counter_pda()), 9);
}

#[test]
fn optional_init_hook_stamps_value_on_creation() {
    let (mut svm, payer) = setup();

    let data = custom_constraints::instruction::HandleOptionalInit {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(optional_counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_optional_init should succeed");

    assert_eq!(read_counter_value(&svm, &optional_counter_pda()), 7);
}

#[test]
fn optional_boxed_init_hook_stamps_value_on_creation() {
    let (mut svm, payer) = setup();

    let data = custom_constraints::instruction::HandleOptionalBoxedInit {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(optional_boxed_counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_optional_boxed_init should succeed");

    assert_eq!(read_counter_value(&svm, &optional_boxed_counter_pda()), 11);
}

// ---- `check` method --------------------------------------------------------

#[test]
fn check_hook_rejects_below_minimum() {
    let (mut svm, payer) = setup();
    init_counter(&mut svm, &payer);
    // After init, value == 5. `handle_check` asserts value >= 10 → fail.

    let data = custom_constraints::instruction::HandleCheck {}.data();
    let metas = vec![AccountMeta::new_readonly(counter_pda(), false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "MinValueConstraint::check should reject value 5 against minimum 10",
    );
}

#[test]
fn check_hook_accepts_at_or_above_minimum() {
    let (mut svm, payer) = setup();
    init_counter(&mut svm, &payer);
    // Bump the counter above the minimum via the update hook first.
    let update_data = custom_constraints::instruction::HandleUpdate {}.data();
    let update_metas = vec![AccountMeta::new(counter_pda(), false)];
    send_instruction(
        &mut svm,
        program_id(),
        update_data,
        update_metas,
        &payer,
        &[],
    )
    .expect("handle_update should succeed");
    assert_eq!(read_counter_value(&svm, &counter_pda()), 42);

    // Now check: 42 >= 10 → should pass.
    let data = custom_constraints::instruction::HandleCheck {}.data();
    let metas = vec![AccountMeta::new_readonly(counter_pda(), false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("MinValueConstraint::check should accept value 42 against minimum 10");
}

// ---- `update` method -------------------------------------------------------

#[test]
fn update_hook_writes_new_value() {
    let (mut svm, payer) = setup();
    init_counter(&mut svm, &payer);
    assert_eq!(read_counter_value(&svm, &counter_pda()), 5);

    let data = custom_constraints::instruction::HandleUpdate {}.data();
    let metas = vec![AccountMeta::new(counter_pda(), false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_update should succeed");

    // `SetValueConstraint::update` overwrote the value.
    assert_eq!(read_counter_value(&svm, &counter_pda()), 42);
}

#[test]
fn update_hook_is_deferred_until_later_field_constraints_pass() {
    let (mut svm, payer) = setup();
    let fresh = fresh_zeroed_pda();
    set_zeroed_counter_account(&mut svm, fresh);

    let data = custom_constraints::instruction::HandleUpdateAfterLaterValidation {}.data();
    let metas = vec![
        AccountMeta::new(fresh, false),
        AccountMeta::new_readonly(payer.pubkey(), true),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("later field constraint should see pre-update value");

    assert_eq!(
        read_counter_value(&svm, &fresh),
        7,
        "update hook should run after later validations succeed",
    );
}

// ---- `exit` method ---------------------------------------------------------

#[test]
fn exit_hook_mutates_on_successful_instruction() {
    let (mut svm, payer) = setup();
    init_counter(&mut svm, &payer);
    assert_eq!(read_counter_value(&svm, &counter_pda()), 5);

    let data = custom_constraints::instruction::HandleExitBump {}.data();
    let metas = vec![AccountMeta::new(counter_pda(), false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_exit_bump should succeed");

    // `BumpOnExitConstraint::exit` added 1 during exit_accounts().
    assert_eq!(read_counter_value(&svm, &counter_pda()), 6);

    // Run it a second time to confirm exit fires on every successful
    // call. LiteSVM re-uses the same blockhash between calls, which
    // would trigger `AlreadyProcessed` on an identical tx — expire
    // before the second send.
    svm.expire_blockhash();
    let data = custom_constraints::instruction::HandleExitBump {}.data();
    let metas = vec![AccountMeta::new(counter_pda(), false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_exit_bump should succeed again");
    assert_eq!(read_counter_value(&svm, &counter_pda()), 7);
}

#[test]
fn boxed_exit_hook_mutates_on_successful_instruction() {
    let (mut svm, payer) = setup();
    init_boxed_counter(&mut svm, &payer);
    assert_eq!(read_counter_value(&svm, &boxed_counter_pda()), 9);

    let data = custom_constraints::instruction::HandleBoxedExitBump {}.data();
    let metas = vec![AccountMeta::new(boxed_counter_pda(), false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_boxed_exit_bump should succeed");

    assert_eq!(read_counter_value(&svm, &boxed_counter_pda()), 11);
}

#[test]
fn boxed_close_transfers_lamports_and_removes_account() {
    let (mut svm, payer) = setup();
    init_boxed_counter(&mut svm, &payer);

    let receiver = keypair_for("boxed-custom-close-receiver");
    svm.airdrop(&receiver.pubkey(), 10_000_000).unwrap();

    let receiver_before = svm.get_account(&receiver.pubkey()).unwrap().lamports;
    let counter_before = svm
        .get_account(&boxed_counter_pda())
        .expect("boxed counter exists")
        .lamports;
    assert!(
        counter_before > 0,
        "boxed counter must hold lamports before close"
    );

    let data = custom_constraints::instruction::HandleBoxedClose {}.data();
    let metas = vec![
        AccountMeta::new(boxed_counter_pda(), false),
        AccountMeta::new(receiver.pubkey(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_boxed_close should succeed");

    let receiver_after = svm.get_account(&receiver.pubkey()).unwrap().lamports;
    assert_eq!(receiver_after, receiver_before + counter_before);

    match svm.get_account(&boxed_counter_pda()) {
        None => {}
        Some(account) => assert_eq!(account.lamports, 0, "closed boxed counter should be empty"),
    }
}

// ---- `init_if_needed` (both `init` and `check` on the create branch) ------

#[test]
fn init_if_needed_create_branch_runs_init_and_check() {
    let (mut svm, payer) = setup();
    // Counter doesn't exist yet. init_if_needed should create it, then
    // `InitValueConstraint::init` stamps value=5, then
    // `MinValueConstraint::check` asserts 5 >= 1 (passes).

    let data = custom_constraints::instruction::HandleInitIfNeeded {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_init_if_needed create branch should succeed");

    assert_eq!(read_counter_value(&svm, &counter_pda()), 5);
}

#[test]
fn optional_init_if_needed_create_branch_runs_init_and_check() {
    let (mut svm, payer) = setup();

    init_optional_if_needed_counter(&mut svm, &payer);

    assert_eq!(
        read_counter_value(&svm, &optional_init_if_needed_counter_pda()),
        13
    );
}

#[test]
fn init_if_needed_exist_branch_runs_check() {
    let (mut svm, payer) = setup();
    // Pre-populate the counter with a value that satisfies the check.
    init_counter(&mut svm, &payer);
    // Bump above the check's minimum so the exist branch can prove check
    // fires against the LIVE value, not against the init-time default.
    let update_data = custom_constraints::instruction::HandleUpdate {}.data();
    let update_metas = vec![AccountMeta::new(counter_pda(), false)];
    send_instruction(
        &mut svm,
        program_id(),
        update_data,
        update_metas,
        &payer,
        &[],
    )
    .expect("handle_update");
    assert_eq!(read_counter_value(&svm, &counter_pda()), 42);

    // Now init_if_needed should take the exist branch: value is 42,
    // the check min=1 passes, init does NOT fire (value stays 42).
    let data = custom_constraints::instruction::HandleInitIfNeeded {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter_pda(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("handle_init_if_needed exist branch should succeed");

    assert_eq!(
        read_counter_value(&svm, &counter_pda()),
        42,
        "exist branch must not re-run init (would have overwritten 42 → 5)",
    );
}

#[test]
fn optional_init_if_needed_exist_branch_runs_check_without_reinit() {
    let (mut svm, payer) = setup();
    init_optional_if_needed_counter(&mut svm, &payer);

    let pda = optional_init_if_needed_counter_pda();
    let mut account = svm.get_account(&pda).expect("counter exists");
    account.data[8..16].copy_from_slice(&42u64.to_le_bytes());
    svm.set_account(pda, account).unwrap();

    svm.expire_blockhash();
    init_optional_if_needed_counter(&mut svm, &payer);

    assert_eq!(
        read_counter_value(&svm, &pda),
        42,
        "exist branch must not re-run init (would have overwritten 42 -> 13)",
    );
}

#[test]
fn init_if_needed_exist_branch_rejects_failed_check_without_reinit() {
    let (mut svm, payer) = setup();
    init_counter(&mut svm, &payer);

    // Corrupt the existing account's live value below the min_value=1
    // check. If the exist branch accidentally re-ran init, it would stamp
    // value=5 and pass; the correct behavior is to check the live data and
    // reject without mutating it.
    let pda = counter_pda();
    let mut account = svm.get_account(&pda).expect("counter exists");
    account.data[8..16].copy_from_slice(&0u64.to_le_bytes());
    svm.set_account(pda, account).unwrap();

    let data = custom_constraints::instruction::HandleInitIfNeeded {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "init_if_needed exist branch must reject when check() fails"
    );
    assert_eq!(
        read_counter_value(&svm, &pda),
        0,
        "failed exist-branch check must not re-run init or mutate data"
    );
}

#[test]
fn optional_init_if_needed_exist_branch_rejects_failed_check_without_reinit() {
    let (mut svm, payer) = setup();
    init_optional_if_needed_counter(&mut svm, &payer);

    let pda = optional_init_if_needed_counter_pda();
    let mut account = svm.get_account(&pda).expect("counter exists");
    account.data[8..16].copy_from_slice(&0u64.to_le_bytes());
    svm.set_account(pda, account).unwrap();

    svm.expire_blockhash();
    let data = custom_constraints::instruction::HandleOptionalInitIfNeeded {}.data();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "optional init_if_needed exist branch must reject when check() fails"
    );
    assert_eq!(
        read_counter_value(&svm, &pda),
        0,
        "failed exist-branch check must not re-run init or mutate data"
    );
}
