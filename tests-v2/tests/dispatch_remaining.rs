use {
    anchor_lang_v2::{
        solana_program::instruction::{AccountMeta, Instruction},
        InstructionData,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "6NxceYZNn23ERJ6rDPENG8iT5bz7osPqiQeWukHaYsRs"
        .parse()
        .unwrap()
}

fn counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"counter"], &program_id()).0
}

const DUPLICATE_MUT_ERROR: u32 = 2005;

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir
            .join("programs/dispatch-remaining")
            .to_str()
            .unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("dispatch_remaining.so"))
        .expect("load dispatch_remaining program");
    let payer = keypair_for("dispatch-remaining-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn initialize(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
    let counter = counter_pda();
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![0], metas, payer, &[])
        .expect("initialize should succeed");
    counter
}

fn call_raw(
    svm: &mut LiteSVM,
    payer: &Keypair,
    data: Vec<u8>,
    accounts: Vec<AccountMeta>,
) -> TransactionResult {
    let ix = Instruction::new_with_bytes(program_id(), &data, accounts);
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

fn counter_value(svm: &LiteSVM, counter: Pubkey) -> u64 {
    let account = svm.get_account(&counter).expect("counter exists");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

#[track_caller]
fn assert_custom_error(result: &TransactionResult, expected: u32) {
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

#[test]
fn too_few_declared_accounts_fails_before_handler() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);

    let result = call_raw(
        &mut svm,
        &payer,
        vec![1],
        vec![AccountMeta::new_readonly(counter, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("NotEnoughAccountKeys") || err.contains("Custom("),
        "too few accounts should be rejected, got: {err}"
    );
}

#[test]
fn remaining_duplicate_of_declared_mut_is_rejected() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let data = dispatch_remaining::instruction::MutateThenReadRemaining { value: 123 }.data();

    let result = call_raw(
        &mut svm,
        &payer,
        data,
        vec![
            AccountMeta::new(counter, false),
            AccountMeta::new(counter, false),
        ],
    );
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn remaining_duplicate_of_declared_readonly_is_allowed() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let data = dispatch_remaining::instruction::ReadRemainingOnce { expected_count: 1 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new_readonly(counter, false),
            AccountMeta::new_readonly(counter, false),
        ],
        &payer,
        &[],
    )
    .expect("readonly declared account may be duplicated through remaining accounts");
}

#[test]
fn remaining_duplicate_of_trailing_account_is_allowed() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let marker = Pubkey::new_unique();

    send_instruction(
        &mut svm,
        program_id(),
        vec![3],
        vec![
            AccountMeta::new_readonly(counter, false),
            AccountMeta::new_readonly(marker, false),
            AccountMeta::new_readonly(marker, false),
        ],
        &payer,
        &[],
    )
    .expect("remaining accounts may duplicate each other when no declared mut slot aliases");
}

#[test]
fn remaining_duplicate_of_optional_some_mut_is_rejected() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let data =
        dispatch_remaining::instruction::OptionalMutThenReadRemaining { expected_count: 1 }.data();

    let result = call_raw(
        &mut svm,
        &payer,
        data,
        vec![
            AccountMeta::new(counter, false),
            AccountMeta::new(counter, false),
        ],
    );
    assert_custom_error(&result, DUPLICATE_MUT_ERROR);
}

#[test]
fn remaining_duplicate_of_optional_none_sentinel_is_allowed() {
    let (mut svm, payer) = setup();
    let data =
        dispatch_remaining::instruction::OptionalMutThenReadRemaining { expected_count: 1 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new(program_id(), false),
            AccountMeta::new(program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("None sentinel should not count as an active optional mut account");
}

#[test]
fn optional_some_mut_with_distinct_remaining_is_allowed() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let marker = Pubkey::new_unique();
    let data =
        dispatch_remaining::instruction::OptionalMutThenReadRemaining { expected_count: 1 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new(counter, false),
            AccountMeta::new_readonly(marker, false),
        ],
        &payer,
        &[],
    )
    .expect("optional Some mut account should allow a distinct remaining account");
}

#[test]
fn extra_accounts_are_available_as_trailing_remaining_region() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let marker = Pubkey::new_unique();
    let data = dispatch_remaining::instruction::ReadRemainingOnce { expected_count: 2 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new_readonly(counter, false),
            AccountMeta::new_readonly(counter, false),
            AccountMeta::new_readonly(marker, false),
        ],
        &payer,
        &[],
    )
    .expect("remaining accounts should be the exact trailing region");
}

#[test]
fn repeated_remaining_calls_return_cached_accounts() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![3],
        vec![
            AccountMeta::new_readonly(counter, false),
            AccountMeta::new_readonly(counter, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
        ],
        &payer,
        &[],
    )
    .expect("remaining_accounts should be stable across repeated calls");
}

#[test]
fn valid_instruction_with_no_remaining_returns_empty_vec() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let data = dispatch_remaining::instruction::ReadRemainingOnce { expected_count: 0 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![AccountMeta::new_readonly(counter, false)],
        &payer,
        &[],
    )
    .expect("no trailing accounts should produce an empty remaining vector");
}

#[test]
fn declared_validation_happens_before_remaining_walk() {
    let (mut svm, payer) = setup();
    let wrong_owner = payer.pubkey();
    let data = dispatch_remaining::instruction::ReadRemainingOnce { expected_count: 2 }.data();

    let result = call_raw(
        &mut svm,
        &payer,
        data,
        vec![
            AccountMeta::new_readonly(wrong_owner, true),
            AccountMeta::new_readonly(wrong_owner, true),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
        ],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("IllegalOwner")
            || err.contains("InvalidAccountData")
            || err.contains("Custom("),
        "declared account validation should fail before remaining handling, got: {err}"
    );
}

#[test]
fn mutate_declared_account_then_read_remaining() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let data = dispatch_remaining::instruction::MutateThenReadRemaining { value: 99 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new(counter, false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
        ],
        &payer,
        &[],
    )
    .expect("declared mutation and remaining read should both work");
    assert_eq!(counter_value(&svm, counter), 99);
}

#[test]
fn unknown_or_short_discriminator_returns_instruction_error() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);

    for data in [vec![250], vec![]] {
        let result = call_raw(
            &mut svm,
            &payer,
            data,
            vec![AccountMeta::new_readonly(counter, false)],
        );
        let err = format!("{:?}", result.unwrap_err().err);
        assert!(
            err.contains("InvalidInstructionData") || err.contains("Custom("),
            "bad discriminator should map to instruction-data error, got: {err}"
        );
    }
}

#[test]
fn malformed_args_fail_after_discriminator_match() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let mut data = dispatch_remaining::instruction::ArgEcho {
        value: 0x0102_0304_0506_0708,
        tag: *b"echo",
    }
    .data();
    data.truncate(5);

    let result = call_raw(
        &mut svm,
        &payer,
        data,
        vec![AccountMeta::new_readonly(counter, false)],
    );
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("InvalidInstructionData") || err.contains("Custom("),
        "truncated args should fail deserialization, got: {err}"
    );
}
