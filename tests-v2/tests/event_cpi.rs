use {
    anchor_lang_v2::{
        event::EVENT_IX_TAG_LE, solana_program::instruction::AccountMeta, InstructionData,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "E2BJE8WXAe7fLnW1ekVGg75udBFfWefNUsvPNcaKwLMm"
        .parse()
        .unwrap()
}

fn counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"event-cpi-counter"], &program_id()).0
}

fn event_authority_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"__event_authority"], &program_id()).0
}

fn event_cpi_ix_data() -> Vec<u8> {
    EVENT_IX_TAG_LE.to_vec()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/event-cpi").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("event_cpi_test.so"))
        .expect("load event_cpi_test program");
    let payer = keypair_for("event-cpi-payer");
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

fn counter_value(svm: &LiteSVM, counter: Pubkey) -> u64 {
    let account = svm.get_account(&counter).expect("counter exists");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

fn assert_error_contains<T, E: std::fmt::Display>(result: Result<T, E>, needle: &str) {
    let Err(error) = result else {
        panic!("expected error containing {needle:?}, got success");
    };
    let error = error.to_string();
    assert!(
        error.contains(needle) || error.contains("Custom("),
        "expected {needle:?}, got:\n{error}"
    );
}

#[test]
fn emit_cpi_from_handler_succeeds() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let event_authority = event_authority_pda();
    let data = event_cpi_test::instruction::EmitOnce { value: 42 }.data();

    send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new(counter, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(program_id(), false),
        ],
        &payer,
        &[],
    )
    .expect("handler should emit event through self-CPI");

    assert_eq!(counter_value(&svm, counter), 42);
}

#[test]
fn emit_cpi_handler_rejects_wrong_event_authority_account() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);
    let data = event_cpi_test::instruction::EmitOnce { value: 42 }.data();

    let result = send_instruction(
        &mut svm,
        program_id(),
        data,
        vec![
            AccountMeta::new(counter, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(program_id(), false),
        ],
        &payer,
        &[],
    );

    assert_error_contains(result, "InvalidSeeds");
    assert_eq!(counter_value(&svm, counter), 0);
}

#[test]
fn direct_event_cpi_dispatch_rejects_missing_accounts() {
    let (mut svm, payer) = setup();

    let result = send_instruction(
        &mut svm,
        program_id(),
        event_cpi_ix_data(),
        vec![],
        &payer,
        &[],
    );

    assert_error_contains(result, "NotEnoughAccountKeys");
}

#[test]
fn direct_event_cpi_dispatch_rejects_unsigned_event_authority_pda() {
    let (mut svm, payer) = setup();
    let event_authority = event_authority_pda();

    let result = send_instruction(
        &mut svm,
        program_id(),
        event_cpi_ix_data(),
        vec![AccountMeta::new_readonly(event_authority, false)],
        &payer,
        &[],
    );

    assert_error_contains(result, "MissingRequiredSignature");
}

#[test]
fn direct_event_cpi_dispatch_rejects_non_pda_signer() {
    let (mut svm, payer) = setup();
    let fake_authority = keypair_for("fake-event-cpi-authority");
    svm.airdrop(&fake_authority.pubkey(), 1_000_000).unwrap();

    let result = send_instruction(
        &mut svm,
        program_id(),
        event_cpi_ix_data(),
        vec![AccountMeta::new_readonly(fake_authority.pubkey(), true)],
        &payer,
        &[&fake_authority],
    );

    assert_error_contains(result, "InvalidSeeds");
}

#[test]
fn full_event_cpi_tag_preempts_overlapping_custom_discriminator() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);

    let result = send_instruction(
        &mut svm,
        program_id(),
        event_cpi_ix_data(),
        vec![AccountMeta::new(counter, false)],
        &payer,
        &[],
    );

    assert_error_contains(result, "MissingRequiredSignature");
    assert_eq!(
        counter_value(&svm, counter),
        0,
        "overlapping handler must not run for the full event-CPI tag"
    );
}

#[test]
fn short_overlapping_custom_discriminator_still_dispatches_normally() {
    let (mut svm, payer) = setup();
    let counter = initialize(&mut svm, &payer);

    send_instruction(
        &mut svm,
        program_id(),
        vec![0xe4],
        vec![AccountMeta::new(counter, false)],
        &payer,
        &[],
    )
    .expect("short custom discriminator should still dispatch");

    assert_eq!(counter_value(&svm, counter), 0xe4);
}
