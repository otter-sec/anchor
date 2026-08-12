//! End-to-end tests for v2 derive/attribute macros not already covered by
//! other suites:
//!   - `#[constant]`, `#[access_control]`, `#[pod_wrapper]`,
//!     `#[derive(InitSpace)]`
//!   - `PodU64` / `PodI32` / `PodBool` runtime behavior on SBF
//!
//! Each test drives a real transaction through the `derives-test` program
//! so the coverage trace attributes hits to the generated code in
//! lang-v2/derive/src/*.rs.

use {
    anchor_lang_v2::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "Der1111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn counter_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"data-v2"], &program_id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/derives").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("derives_test.so"))
        .expect("load derives_test program");
    let payer = keypair_for("derives-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn do_initialize(svm: &mut LiteSVM, payer: &Keypair) -> Pubkey {
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

fn do_bump(svm: &mut LiteSVM, payer: &Keypair, counter: Pubkey, amount: u64, step: i32) {
    let mut data = vec![1];
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&step.to_le_bytes());
    let metas = vec![AccountMeta::new(counter, false)];
    send_instruction(svm, program_id(), data, metas, payer, &[]).expect("bump should succeed");
}

// Layout of the `Counter` slab account after the 8-byte Anchor discriminator:
//   offset 8   : PodU64  value   (8 bytes)
//   offset 16  : PodI32  delta   (4 bytes)
//   offset 20  : PodBool active  (1 byte)
//   offset 21  : PodMode mode    (1 byte, pod_wrapper u8)
const OFFSET_VALUE: usize = 8;
const OFFSET_DELTA: usize = 16;
const OFFSET_ACTIVE: usize = 20;
const OFFSET_MODE: usize = 21;

fn read_value(svm: &LiteSVM, counter: &Pubkey) -> u64 {
    let data = &svm.get_account(counter).unwrap().data;
    u64::from_le_bytes(data[OFFSET_VALUE..OFFSET_VALUE + 8].try_into().unwrap())
}

fn read_delta(svm: &LiteSVM, counter: &Pubkey) -> i32 {
    let data = &svm.get_account(counter).unwrap().data;
    i32::from_le_bytes(data[OFFSET_DELTA..OFFSET_DELTA + 4].try_into().unwrap())
}

fn read_active(svm: &LiteSVM, counter: &Pubkey) -> u8 {
    svm.get_account(counter).unwrap().data[OFFSET_ACTIVE]
}

fn read_mode(svm: &LiteSVM, counter: &Pubkey) -> u8 {
    svm.get_account(counter).unwrap().data[OFFSET_MODE]
}

// ---- #[constant] ----------------------------------------------------------

#[test]
fn constant_seed_round_trips_through_pda_derivation() {
    // `DATA_SEED` is the only way the test knows which PDA to pass; if the
    // `#[constant]` attribute silently dropped the item, compilation of
    // `programs/derives` wouldn't have produced a valid PDA and the
    // init handler would fail.
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    assert!(svm.get_account(&counter).is_some());
}

// ---- PodU64 / PodI32 / PodBool runtime behavior on SBF -------------------

#[test]
fn pod_arithmetic_writes_expected_bytes() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // Post-init: value=0, delta=0, active=0 (false), mode=0 (Idle).
    assert_eq!(read_value(&svm, &counter), 0);
    assert_eq!(read_delta(&svm, &counter), 0);
    assert_eq!(read_active(&svm, &counter), 0);
    assert_eq!(read_mode(&svm, &counter), 0);

    // value += 42, delta += -7, active := true, mode := Active (1).
    do_bump(&mut svm, &payer, counter, 42, -7);
    assert_eq!(read_value(&svm, &counter), 42);
    assert_eq!(read_delta(&svm, &counter), -7);
    assert_eq!(read_active(&svm, &counter), 1);
    assert_eq!(read_mode(&svm, &counter), 1);

    // Accumulates across calls.
    do_bump(&mut svm, &payer, counter, 8, 3);
    assert_eq!(read_value(&svm, &counter), 50);
    assert_eq!(read_delta(&svm, &counter), -4);
}

#[test]
fn verify_invariants_passes_after_bump() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    do_bump(&mut svm, &payer, counter, 5, 0);

    // discrim=2 exercises checked_add, saturating_sub, PartialEq<u64>,
    // PartialOrd, and the PodMode↔Mode cross-type PartialEq.
    let metas = vec![AccountMeta::new(counter, false)];
    send_instruction(&mut svm, program_id(), vec![2], metas, &payer, &[])
        .expect("invariants should hold post-bump");
}

#[test]
fn verify_invariants_rejects_zero_value() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // Without a bump, value == 0 — the handler's first comparison fails.
    let metas = vec![AccountMeta::new(counter, false)];
    let result = send_instruction(&mut svm, program_id(), vec![2], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "invariants should fail when value == 0 (Idle mode)"
    );
}

#[test]
fn verify_invariants_rejects_invalid_pod_wrapper_discriminant() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    do_bump(&mut svm, &payer, counter, 5, 0);

    let mut account = svm.get_account(&counter).expect("counter exists");
    account.data[OFFSET_MODE] = 42;
    svm.set_account(counter, account).unwrap();

    let metas = vec![AccountMeta::new(counter, false)];
    let result = send_instruction(&mut svm, program_id(), vec![2], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "invalid PodMode byte must not silently pass equality checks"
    );
}

// ---- #[access_control] ---------------------------------------------------

#[test]
fn access_control_allows_matching_authority() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    let authority = keypair_for("authority");

    // privileged (discrim=3, amount=100). Authority must equal `expected`
    // AND amount must be under MAX_DEPTH * 1000 = 7000.
    let mut data = vec![3];
    data.extend_from_slice(&100u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(authority.pubkey(), false), // expected == authority
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority])
        .expect("privileged should succeed with matching authority");

    assert_eq!(read_value(&svm, &counter), 100);
}

#[test]
fn access_control_rejects_mismatched_authority() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    let authority = keypair_for("authority");
    let imposter = keypair_for("imposter");

    let mut data = vec![3];
    data.extend_from_slice(&100u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(imposter.pubkey(), false), // expected != authority
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority]);
    assert!(result.is_err(), "mismatched authority should be rejected");
}

#[test]
fn access_control_rejects_amount_above_cap() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    let authority = keypair_for("authority");

    // amount = 10_000 > MAX_DEPTH(7) * 1000 — the second access_control
    // check fires after the first one passes.
    let mut data = vec![3];
    data.extend_from_slice(&10_000u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(counter, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(authority.pubkey(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority]);
    assert!(result.is_err(), "amount above cap should be rejected");
}

// ---- #[derive(InitSpace)] -------------------------------------------------

#[test]
fn init_profile_allocates_init_space_bytes() {
    let (mut svm, payer) = setup();

    // Profile layout (borsh): owner(32) + tier(1) + scores(4 + 4*8 = 36) = 69.
    // The init handler uses `8 + Profile::INIT_SPACE` — if the derive computed
    // a wrong value, allocation and/or borsh serialize would fail at runtime.
    let profile_pda = Pubkey::find_program_address(&[b"profile"], &program_id()).0;
    let mut data = vec![4];
    data.push(3); // tier
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(profile_pda, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("init_profile should succeed");

    let account = svm.get_account(&profile_pda).expect("profile exists");
    assert_eq!(account.data.len(), 8 + 69);

    // Verify the borsh payload round-trips: discriminator + owner + tier=3
    // + scores=[10,20,30,40].
    let tier = account.data[8 + 32];
    assert_eq!(tier, 3);
    // Vec length at offset 8 + 32 + 1.
    let vec_len = u32::from_le_bytes(account.data[8 + 33..8 + 37].try_into().unwrap());
    assert_eq!(vec_len, 4);
    let first_score = u64::from_le_bytes(account.data[8 + 37..8 + 45].try_into().unwrap());
    assert_eq!(first_score, 10);
}

// ---- #[event] + emit! ----------------------------------------------------

/// Compute the 8-byte event discriminator the macro generates: first 8
/// bytes of sha256(`"event:<StructName>"`).
fn event_discriminator(name: &str) -> [u8; 8] {
    use sha2::Digest;
    let full = sha2::Sha256::digest(format!("event:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&full[..8]);
    out
}

/// Decode a `Program data: <base64>` log line into its raw bytes.
fn program_data_from_logs(logs: &[String]) -> Vec<u8> {
    let line = logs
        .iter()
        .find(|l| l.starts_with("Program data: "))
        .expect("should have a `Program data:` log line");
    let b64 = line.trim_start_matches("Program data: ").trim();
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).expect("base64 decode")
}

#[test]
fn emit_wincode_event_logs_program_data() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);

    // bump with amount=12345, step=-7. `emit!` inside the handler fires a
    // wincode-serialized Bumped event.
    let mut data = vec![1];
    data.extend_from_slice(&12345u64.to_le_bytes());
    data.extend_from_slice(&(-7i32).to_le_bytes());
    let metas = vec![AccountMeta::new(counter, false)];
    let meta = send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("bump should succeed");

    let bytes = program_data_from_logs(&meta.logs);
    assert!(
        bytes.len() >= 8,
        "event payload should include discriminator"
    );
    assert_eq!(
        &bytes[..8],
        &event_discriminator("Bumped"),
        "discriminator mismatch"
    );

    // Default-mode event uses wincode with a borsh-compatible wire format:
    // u64 LE (8) + i32 LE (4) + bool (1 byte). Total = 21 including disc.
    assert_eq!(bytes.len(), 21);
    let amount = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let step = i32::from_le_bytes(bytes[16..20].try_into().unwrap());
    let flag = bytes[20];
    assert_eq!(amount, 12345);
    assert_eq!(step, -7);
    assert_eq!(flag, 1);
}

#[test]
fn emit_bytemuck_event_copies_repr_c_bytes() {
    let (mut svm, payer) = setup();
    let counter = do_initialize(&mut svm, &payer);
    // Bump first so `value` and `mode` are non-zero when snapshot runs.
    do_bump(&mut svm, &payer, counter, 99, 0);

    // snapshot (discrim=5) emits a bytemuck-mode event.
    let metas = vec![AccountMeta::new(counter, false)];
    let meta = send_instruction(&mut svm, program_id(), vec![5], metas, &payer, &[])
        .expect("snapshot should succeed");

    let bytes = program_data_from_logs(&meta.logs);
    assert_eq!(
        &bytes[..8],
        &event_discriminator("SnapshotTaken"),
        "discriminator mismatch"
    );

    // Bytemuck layout: repr(C) struct bytes verbatim.
    // SnapshotTaken { value: u64 (8), mode: u8 (1), _pad: [u8; 7] (7) } = 16
    // bytes. Total payload = 8 (disc) + 16 = 24.
    assert_eq!(bytes.len(), 24);
    let value = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    let mode = bytes[16];
    assert_eq!(value, 99);
    assert_eq!(mode, 1, "mode = PodMode::Active after bump");
}
