use {
    anchor_lang::solana_program::instruction::{AccountMeta, Instruction},
    litesvm::{types::TransactionResult, LiteSVM},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "EDhJyPDycxByBe3wTsN2zppGcRYgM2WR5LQw9f8SFxMF"
        .parse()
        .unwrap()
}

fn data_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"data"], &program_id()).0
}

fn maybe_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"maybe"], &program_id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir
            .join("programs/optional-accounts")
            .to_str()
            .unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("optional_accounts.so"))
        .expect("load optional_accounts program");
    let payer = keypair_for("optional-accounts-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn call_raw(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    accounts: Vec<AccountMeta>,
) -> TransactionResult {
    let ix = Instruction::new_with_bytes(program_id(), &[disc], accounts);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

fn init_required(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let data = data_pda();
    send_instruction(
        svm,
        program_id(),
        vec![0],
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        payer,
        &[],
    )
    .expect("init_required should succeed");
    data
}

fn data_value(svm: &LiteSVM, pubkey: Pubkey) -> u64 {
    let account = svm.get_account(&pubkey).expect("data account exists");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

fn set_program_owned_account(svm: &mut LiteSVM, pubkey: Pubkey, data: Vec<u8>) {
    svm.set_account(
        pubkey,
        Account {
            lamports: 10_000_000,
            data,
            owner: program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

#[track_caller]
fn assert_no_spy_load_logs(logs: &str) {
    assert!(
        !logs.contains("spy_load_mut"),
        "duplicate rejection should happen before load_mut, logs were:\n{logs}",
    );
}

#[test]
fn program_id_sentinel_maps_optional_to_none() {
    let (mut svm, payer) = setup();
    send_instruction(
        &mut svm,
        program_id(),
        vec![1],
        vec![AccountMeta::new_readonly(program_id(), false)],
        &payer,
        &[],
    )
    .expect("program id sentinel should load as None");
}

#[test]
fn valid_optional_some_loads_and_wrong_non_sentinel_fails() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![1],
        vec![AccountMeta::new_readonly(data, false)],
        &payer,
        &[],
    )
    .expect("valid optional Some should load");

    let result = call_raw(
        &mut svm,
        &payer,
        1,
        vec![AccountMeta::new_readonly(payer.pubkey(), true)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("IllegalOwner")
            || err.contains("InvalidAccountData")
            || err.contains("Custom("),
        "wrong non-sentinel account should fail normal Account<T> validation, got: {err}"
    );
}

#[test]
fn mutable_optional_duplicate_check_is_gated_on_some() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![2],
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new(program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("None sentinel should not trip optional duplicate-mut check");
    assert_eq!(data_value(&svm, data), 8);

    let result = call_raw(
        &mut svm,
        &payer,
        2,
        vec![AccountMeta::new(data, false), AccountMeta::new(data, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("Duplicate") || err.contains("Custom("),
        "Some alias should trip optional duplicate-mut check, got: {err}"
    );
}

#[test]
fn duplicate_optional_some_rejects_before_any_spy_load_mut() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    let failure = call_raw(
        &mut svm,
        &payer,
        10,
        vec![AccountMeta::new(data, false), AccountMeta::new(data, false)],
    )
    .expect_err("duplicate Some should be rejected");
    let err = format!("{:?}", failure.err);
    assert!(
        err.contains("Duplicate") || err.contains("Custom("),
        "duplicate Some should trip optional duplicate-mut check, got: {err}"
    );
    assert_no_spy_load_logs(&failure.meta.pretty_logs());
}

#[test]
fn duplicate_optional_none_sentinels_skip_spy_load_mut() {
    let (mut svm, payer) = setup();

    let meta = call_raw(
        &mut svm,
        &payer,
        10,
        vec![
            AccountMeta::new(program_id(), false),
            AccountMeta::new(program_id(), false),
        ],
    )
    .expect("duplicate None sentinels should stay silent");
    assert_no_spy_load_logs(&meta.pretty_logs());
}

#[test]
fn double_optional_none_sentinels_stay_silent() {
    let (mut svm, payer) = setup();

    send_instruction(
        &mut svm,
        program_id(),
        vec![11],
        vec![
            AccountMeta::new(program_id(), false),
            AccountMeta::new(program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("duplicate None sentinels should stay silent across optional mut fields");
}

#[test]
fn double_optional_duplicate_some_still_fails() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    let result = call_raw(
        &mut svm,
        &payer,
        11,
        vec![AccountMeta::new(data, false), AccountMeta::new(data, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("Duplicate") || err.contains("Custom("),
        "duplicate Some alias should still fail, got: {err}"
    );
}

#[test]
fn optional_seed_bumps_are_some_only_for_some_accounts() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![3],
        vec![AccountMeta::new_readonly(program_id(), false)],
        &payer,
        &[],
    )
    .expect("None should skip seeds and leave bump None");
    send_instruction(
        &mut svm,
        program_id(),
        vec![3],
        vec![AccountMeta::new_readonly(data, false)],
        &payer,
        &[],
    )
    .expect("Some should verify seeds and assign bump Some");
}

#[test]
fn explicit_bump_rejects_corrupted_some_but_skips_none() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![4],
        vec![AccountMeta::new_readonly(program_id(), false)],
        &payer,
        &[],
    )
    .expect("None should skip explicit bump check");

    send_instruction(
        &mut svm,
        program_id(),
        vec![5],
        vec![AccountMeta::new(data, false)],
        &payer,
        &[],
    )
    .expect("corrupt bump");
    let result = call_raw(
        &mut svm,
        &payer,
        4,
        vec![AccountMeta::new_readonly(data, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("InvalidSeeds") || err.contains("Custom("),
        "corrupted explicit bump should fail seed verification, got: {err}"
    );
}

#[test]
fn init_if_needed_creates_reuses_and_skips_none() {
    let (mut svm, payer) = setup();
    let maybe = maybe_pda();

    send_instruction(
        &mut svm,
        program_id(),
        vec![6],
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_id(), false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("None should skip init_if_needed");
    assert!(svm.get_account(&maybe).is_none());

    send_instruction(
        &mut svm,
        program_id(),
        vec![6],
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(maybe, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("Some missing should create account");
    assert_eq!(data_value(&svm, maybe), 1);

    svm.expire_blockhash();
    send_instruction(
        &mut svm,
        program_id(),
        vec![6],
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(maybe, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("Some existing should reuse account");
    assert_eq!(data_value(&svm, maybe), 2);
}

#[test]
fn init_if_needed_rejects_optional_existing_account_with_wrong_space() {
    let (mut svm, payer) = setup();
    let maybe = maybe_pda();

    send_instruction(
        &mut svm,
        program_id(),
        vec![6],
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(maybe, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[],
    )
    .expect("Some missing should create account");
    assert_eq!(data_value(&svm, maybe), 1);

    let mut account = svm.get_account(&maybe).expect("maybe account exists");
    account.data.extend_from_slice(&[0u8; 8]);
    svm.set_account(maybe, account)
        .expect("grow maybe account data");
    svm.expire_blockhash();

    let result = call_raw(
        &mut svm,
        &payer,
        6,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(maybe, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("Custom(2019)"),
        "mismatched optional init_if_needed space should be rejected, got: {err}"
    );
    assert_eq!(
        data_value(&svm, maybe),
        1,
        "rejected reuse should not mutate the existing optional account",
    );
}

#[test]
fn zeroed_optional_initializes_some_and_skips_none() {
    let (mut svm, payer) = setup();
    let zeroed = Pubkey::new_unique();
    set_program_owned_account(&mut svm, zeroed, vec![0u8; 24]);

    send_instruction(
        &mut svm,
        program_id(),
        vec![7],
        vec![AccountMeta::new(program_id(), false)],
        &payer,
        &[],
    )
    .expect("None should skip zeroed");
    send_instruction(
        &mut svm,
        program_id(),
        vec![7],
        vec![AccountMeta::new(zeroed, false)],
        &payer,
        &[],
    )
    .expect("zeroed Some should initialize account");
    assert_eq!(data_value(&svm, zeroed), 11);
}

#[test]
fn close_optional_some_transfers_lamports_and_none_is_noop() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);
    let receiver = keypair_for("optional-close-receiver");
    svm.airdrop(&receiver.pubkey(), 1_000_000).unwrap();
    let before_none = svm.get_account(&receiver.pubkey()).unwrap().lamports;

    send_instruction(
        &mut svm,
        program_id(),
        vec![8],
        vec![
            AccountMeta::new(program_id(), false),
            AccountMeta::new(receiver.pubkey(), false),
        ],
        &payer,
        &[],
    )
    .expect("None close should be a no-op");
    assert_eq!(
        svm.get_account(&receiver.pubkey()).unwrap().lamports,
        before_none
    );

    let data_lamports = svm.get_account(&data).unwrap().lamports;
    send_instruction(
        &mut svm,
        program_id(),
        vec![8],
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new(receiver.pubkey(), false),
        ],
        &payer,
        &[],
    )
    .expect("Some close should transfer lamports");
    let after = svm.get_account(&receiver.pubkey()).unwrap().lamports;
    assert_eq!(after, before_none + data_lamports);
}

#[test]
fn constraints_are_skipped_on_none_and_enforced_on_some() {
    let (mut svm, payer) = setup();
    let data = init_required(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![9],
        vec![AccountMeta::new_readonly(program_id(), false)],
        &payer,
        &[],
    )
    .expect("None should skip raw constraints");
    send_instruction(
        &mut svm,
        program_id(),
        vec![9],
        vec![AccountMeta::new_readonly(data, false)],
        &payer,
        &[],
    )
    .expect("Some satisfying constraint should pass");

    send_instruction(
        &mut svm,
        program_id(),
        vec![2],
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new(program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("mutate value away from constraint");
    svm.expire_blockhash();
    let result = call_raw(
        &mut svm,
        &payer,
        9,
        vec![AccountMeta::new_readonly(data, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("InvalidAccountData") || err.contains("Custom("),
        "Some failing constraint should be enforced, got: {err}"
    );
}
