//! Integration tests for the derive's account-constraint surface.
//!
//! One happy-path + one violation-path per constraint listed in
//! `programs/constraints/src/lib.rs`. Violations that surface an
//! `ErrorCode::*` variant assert the known `Custom(N)` code (or the
//! corresponding `ProgramError` variant). `@ MyErr` variants assert the
//! custom-enum code rather than the built-in.

use {
    anchor_lang_v2::solana_program::instruction::{AccountMeta, Instruction},
    litesvm::{types::TransactionResult, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

// ---- Constants + custom error codes ----------------------------------------
//
// `#[error_code]` lays out variants starting at 6000 by default, one per
// variant in source order. `MyErr::BadAddress` = 6000, `BadAuthority` =
// 6001, `BadOwner` = 6002, `BadConstraint` = 6003.

const ERR_BAD_ADDRESS: u32 = 6000;
const ERR_BAD_AUTHORITY: u32 = 6001;
const ERR_BAD_OWNER: u32 = 6002;
const ERR_BAD_CONSTRAINT: u32 = 6003;
const ERR_BAD_FIRST: u32 = 6004;
const ERR_BAD_SECOND: u32 = 6005;

/// `ErrorCode::ConstraintRaw` maps to `Custom(2001)`.
const CONSTRAINT_RAW: u32 = 2001;
/// `ErrorCode::ConstraintExecutable` maps to `Custom(2002)`.
const CONSTRAINT_EXECUTABLE: u32 = 2002;
/// `ErrorCode::ConstraintZero` maps to `Custom(2004)`.
const CONSTRAINT_ZERO: u32 = 2004;

// Matching the pinned address baked into the program.
fn pinned_address() -> Pubkey {
    "Pin1111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn other_program() -> Pubkey {
    "Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u"
        .parse()
        .unwrap()
}

fn program_id() -> Pubkey {
    "Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp"
        .parse()
        .unwrap()
}

fn data_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"data"], &program_id()).0
}

fn maybe_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"maybe"], &program_id()).0
}

fn maybe_explicit_bump_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"maybe-explicit"], &program_id()).0
}

fn foreign_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"foreign"], &program_id()).0
}

fn other_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"other"], &other_program()).0
}

// ---- Setup -----------------------------------------------------------------

fn setup() -> (LiteSVM, Keypair, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/constraints").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("constraints.so"))
        .expect("deploy");

    let payer = keypair_for("constraints-payer");
    let authority = keypair_for("constraints-authority");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();

    (svm, payer, authority)
}

// ---- Helpers ---------------------------------------------------------------

fn call(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    accounts: Vec<AccountMeta>,
    signers: &[&Keypair],
) -> anyhow::Result<()> {
    send_instruction(svm, program_id(), vec![disc], accounts, payer, signers)?;
    Ok(())
}

fn call_raw(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    accounts: Vec<AccountMeta>,
    signers: &[&Keypair],
) -> TransactionResult {
    let ix = Instruction::new_with_bytes(program_id(), &[disc], accounts);
    let blockhash = svm.latest_blockhash();
    let message = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let mut all_signers: Vec<&dyn solana_signer::Signer> = vec![payer];
    for s in signers {
        all_signers.push(*s);
    }
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(message), &all_signers)
        .expect("sign");
    svm.send_transaction(tx)
}

#[track_caller]
fn assert_custom(result: &TransactionResult, expected: u32) {
    let failure = match result {
        Ok(_) => panic!("expected Custom({expected}), got success"),
        Err(f) => f,
    };
    let rendered = format!("{:?}", failure.err);
    assert!(
        rendered.contains(&format!("Custom({expected})")),
        "expected Custom({expected}), got: {rendered}",
    );
}

#[track_caller]
fn assert_err_contains(result: &TransactionResult, needle: &str) {
    let failure = match result {
        Ok(_) => panic!("expected error containing {needle:?}, got success"),
        Err(f) => f,
    };
    let rendered = format!("{:?}", failure.err);
    assert!(
        rendered.contains(needle),
        "expected {needle:?} in error, got: {rendered}",
    );
}

/// Deploy a `Data` PDA whose `authority` field equals `authority_key`.
fn init_data(svm: &mut LiteSVM, payer: &Keypair, authority_key: &Pubkey) -> Pubkey {
    let data = data_pda();
    call(
        svm,
        payer,
        0,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(*authority_key, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("initialize");
    data
}

// ---- 1. address = expr -----------------------------------------------------

#[test]
fn address_match_ok() {
    let (mut svm, payer, _) = setup();
    call(
        &mut svm,
        &payer,
        1,
        vec![AccountMeta::new_readonly(pinned_address(), false)],
        &[],
    )
    .expect("address match");
}

#[test]
fn address_mismatch_rejected() {
    let (mut svm, payer, _) = setup();
    let wrong = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        1,
        vec![AccountMeta::new_readonly(wrong, false)],
        &[],
    );
    // Default `ConstraintAddress` maps to `ProgramError::InvalidAccountData`.
    assert_err_contains(&result, "InvalidAccountData");
}

// ---- 2. address = expr @ MyErr --------------------------------------------

#[test]
fn address_custom_err_match_ok() {
    let (mut svm, payer, _) = setup();
    call(
        &mut svm,
        &payer,
        2,
        vec![AccountMeta::new_readonly(pinned_address(), false)],
        &[],
    )
    .expect("address match");
}

#[test]
fn address_custom_err_mismatch_surfaces_custom() {
    let (mut svm, payer, _) = setup();
    let wrong = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        2,
        vec![AccountMeta::new_readonly(wrong, false)],
        &[],
    );
    assert_custom(&result, ERR_BAD_ADDRESS);
}

// ---- 3. has_one = field ----------------------------------------------------

#[test]
fn has_one_match_ok() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());
    call(
        &mut svm,
        &payer,
        3,
        vec![
            AccountMeta::new_readonly(data, false),
            AccountMeta::new_readonly(authority.pubkey(), false),
        ],
        &[],
    )
    .expect("has_one match");
}

#[test]
fn has_one_mismatch_rejected() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());
    let wrong = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        3,
        vec![
            AccountMeta::new_readonly(data, false),
            AccountMeta::new_readonly(wrong, false),
        ],
        &[],
    );
    // Default `ConstraintHasOne` -> `ProgramError::InvalidAccountData`.
    assert_err_contains(&result, "InvalidAccountData");
}

// ---- 4. has_one = field @ MyErr -------------------------------------------

#[test]
fn has_one_custom_err_mismatch_surfaces_custom() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());
    let wrong = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        4,
        vec![
            AccountMeta::new_readonly(data, false),
            AccountMeta::new_readonly(wrong, false),
        ],
        &[],
    );
    assert_custom(&result, ERR_BAD_AUTHORITY);
}

// ---- 3b. address = <sibling>.<self> (v2 replacement for `has_one`) -------
//
// Same runtime invariant as handler 3's `has_one`, reached via the
// `address` codegen branch. Worth testing independently because the
// paths share neither emission nor error surface.

#[test]
fn address_field_path_match_ok() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());
    call(
        &mut svm,
        &payer,
        15,
        vec![
            AccountMeta::new_readonly(data, false),
            AccountMeta::new_readonly(authority.pubkey(), false),
        ],
        &[],
    )
    .expect("address = data.authority match");
}

#[test]
fn address_field_path_mismatch_rejected() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());
    let wrong = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        15,
        vec![
            AccountMeta::new_readonly(data, false),
            AccountMeta::new_readonly(wrong, false),
        ],
        &[],
    );
    // Default `ConstraintAddress` -> `ProgramError::InvalidAccountData`.
    assert_err_contains(&result, "InvalidAccountData");
}

// ---- 5. owner = expr -------------------------------------------------------

#[test]
fn owner_match_ok() {
    let (mut svm, payer, _) = setup();
    // payer is system-owned, so `owner = System::id()` holds.
    call(
        &mut svm,
        &payer,
        5,
        vec![AccountMeta::new_readonly(payer.pubkey(), false)],
        &[],
    )
    .expect("owner match");
}

#[test]
fn owner_mismatch_rejected() {
    let (mut svm, payer, _) = setup();
    // Pass the program id itself — owned by the BPF loader, not system.
    let result = call_raw(
        &mut svm,
        &payer,
        5,
        vec![AccountMeta::new_readonly(program_id(), false)],
        &[],
    );
    // `ConstraintOwner` -> `ProgramError::IllegalOwner`.
    assert_err_contains(&result, "IllegalOwner");
}

// ---- 6. owner = expr @ MyErr ----------------------------------------------

#[test]
fn owner_custom_err_mismatch_surfaces_custom() {
    let (mut svm, payer, _) = setup();
    let result = call_raw(
        &mut svm,
        &payer,
        6,
        vec![AccountMeta::new_readonly(program_id(), false)],
        &[],
    );
    assert_custom(&result, ERR_BAD_OWNER);
}

#[test]
fn init_with_owner_creates_foreign_owned_unchecked_account() {
    let (mut svm, payer, _) = setup();
    let foreign = foreign_pda();

    call(
        &mut svm,
        &payer,
        20,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(foreign, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("init foreign-owned unchecked account");

    let account = svm.get_account(&foreign).expect("created account");
    assert_eq!(account.owner, other_program());
    assert_eq!(account.data.len(), 8);
}

// ---- 7. constraint = expr --------------------------------------------------

#[test]
fn constraint_true_ok() {
    let (mut svm, payer, _) = setup();
    let a = Pubkey::new_unique();
    let b = Pubkey::new_unique();
    call(
        &mut svm,
        &payer,
        7,
        vec![
            AccountMeta::new_readonly(a, false),
            AccountMeta::new_readonly(b, false),
        ],
        &[],
    )
    .expect("distinct addresses");
}

#[test]
fn constraint_false_rejected() {
    let (mut svm, payer, _) = setup();
    let dup = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        7,
        vec![
            AccountMeta::new_readonly(dup, false),
            AccountMeta::new_readonly(dup, false),
        ],
        &[],
    );
    assert_custom(&result, CONSTRAINT_RAW);
}

// ---- 8. constraint = expr @ MyErr -----------------------------------------

#[test]
fn constraint_custom_err_false_surfaces_custom() {
    let (mut svm, payer, _) = setup();
    let dup = Pubkey::new_unique();
    let result = call_raw(
        &mut svm,
        &payer,
        8,
        vec![
            AccountMeta::new_readonly(dup, false),
            AccountMeta::new_readonly(dup, false),
        ],
        &[],
    );
    assert_custom(&result, ERR_BAD_CONSTRAINT);
}

// ---- 9. executable ---------------------------------------------------------

#[test]
fn executable_ok_with_program_id() {
    let (mut svm, payer, _) = setup();
    // The program's own account is executable. Passing it to an
    // `#[account(executable)]` field should pass.
    call(
        &mut svm,
        &payer,
        9,
        vec![AccountMeta::new_readonly(program_id(), false)],
        &[],
    )
    .expect("program-id is executable");
}

#[test]
fn executable_rejected_on_non_executable() {
    let (mut svm, payer, _) = setup();
    // payer is a plain system-owned wallet — not executable.
    let result = call_raw(
        &mut svm,
        &payer,
        9,
        vec![AccountMeta::new_readonly(payer.pubkey(), false)],
        &[],
    );
    assert_custom(&result, CONSTRAINT_EXECUTABLE);
}

// ---- 10. close = receiver --------------------------------------------------

/// `Data` layout on-disk: disc(8) + authority([u8; 32]) + value(u64).
/// `value` lives at bytes 40..48.
fn read_value(svm: &LiteSVM, pda: &Pubkey) -> Option<u64> {
    let account = svm.get_account(pda)?;
    if account.data.len() < 48 {
        return None;
    }
    Some(u64::from_le_bytes(account.data[40..48].try_into().unwrap()))
}

/// `DataWithBump` layout: disc(8) + value(u64) + bump(u8).
fn read_value_and_bump(svm: &LiteSVM, pda: &Pubkey) -> Option<(u64, u8)> {
    let account = svm.get_account(pda)?;
    if account.data.len() < 17 {
        return None;
    }
    Some((
        u64::from_le_bytes(account.data[8..16].try_into().unwrap()),
        account.data[16],
    ))
}

#[test]
fn close_transfers_lamports_and_zeros_account() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());

    let receiver = keypair_for("close-receiver");
    // The receiver starts at 0 lamports — it does not need to exist
    // on-chain before the close, since the CPI just credits lamports to
    // the AccountInfo.
    let receiver_before = svm
        .get_account(&receiver.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);
    let data_before = svm.get_account(&data).unwrap().lamports;
    assert!(data_before > 0, "data PDA must hold lamports before close");

    call(
        &mut svm,
        &payer,
        10,
        vec![
            AccountMeta::new(data, false),
            AccountMeta::new(receiver.pubkey(), false),
        ],
        &[],
    )
    .expect("close");

    // Receiver got the full balance.
    let receiver_after = svm.get_account(&receiver.pubkey()).unwrap().lamports;
    assert_eq!(
        receiver_after,
        receiver_before + data_before,
        "receiver should receive the closed account's lamports",
    );

    // Closed account: either absent or zero lamports + system-owned.
    match svm.get_account(&data) {
        None => {}
        Some(a) => {
            assert_eq!(a.lamports, 0, "closed account should have zero lamports");
        }
    }
}

#[test]
fn close_self_close_rejected() {
    let (mut svm, payer, authority) = setup();
    let data = init_data(&mut svm, &payer, &authority.pubkey());

    // Pass `data` as both `data` and `receiver`. Both slots are `mut`
    // in `DoClose`, so the duplicate-mutable-account guard fires first
    // (`Custom(2005)`), before the derive's self-close check would run.
    // Both are derive-level rejections of the same misuse — accept
    // either. (The self-close check only becomes reachable if the
    // receiver slot is read-only, which is not a legitimate close.)
    let result = call_raw(
        &mut svm,
        &payer,
        10,
        vec![AccountMeta::new(data, false), AccountMeta::new(data, false)],
        &[],
    );
    let rendered = format!("{:?}", result.as_ref().err().expect("should fail").err);
    assert!(
        rendered.contains("Custom(2005)") || rendered.contains("InvalidAccountData"),
        "expected dup-mut or ConstraintClose rejection, got: {rendered}",
    );
}

// ---- 11. seeds::program = OTHER_PROGRAM -----------------------------------

#[test]
fn seeds_program_override_ok() {
    let (mut svm, payer, _) = setup();
    call(
        &mut svm,
        &payer,
        11,
        vec![AccountMeta::new_readonly(other_pda(), false)],
        &[],
    )
    .expect("PDA derived under OTHER_PROGRAM");
}

#[test]
fn seeds_program_override_wrong_pda_rejected() {
    let (mut svm, payer, _) = setup();
    // Same seed, but derived under THIS program id — should fail because
    // the derive verifies under OTHER_PROGRAM.
    let wrong_pda = Pubkey::find_program_address(&[b"other"], &program_id()).0;
    let result = call_raw(
        &mut svm,
        &payer,
        11,
        vec![AccountMeta::new_readonly(wrong_pda, false)],
        &[],
    );
    // `ConstraintSeeds` -> `ProgramError::InvalidSeeds`.
    assert_err_contains(&result, "InvalidSeeds");
}

// ---- 12. init_if_needed ---------------------------------------------------

#[test]
fn init_if_needed_creates_then_reuses() {
    let (mut svm, payer, _) = setup();
    let pda = maybe_pda();

    // Account must not yet exist.
    assert!(svm
        .get_account(&pda)
        .map(|a| a.data.is_empty())
        .unwrap_or(true));

    // First call: init path.
    call(
        &mut svm,
        &payer,
        12,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("first init_if_needed call");
    assert_eq!(read_value(&svm, &pda), Some(1));

    // Advance the blockhash so the second tx has a distinct signature;
    // otherwise LiteSVM rejects it as `AlreadyProcessed`.
    svm.expire_blockhash();

    // Second call: should NOT re-init — must reuse and just bump the value.
    call(
        &mut svm,
        &payer,
        12,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("second init_if_needed call");
    assert_eq!(
        read_value(&svm, &pda),
        Some(2),
        "second call must reuse account, not reset its data",
    );
}

#[test]
fn init_if_needed_allows_explicit_bump_on_reuse() {
    let (mut svm, payer, _) = setup();
    let pda = maybe_explicit_bump_pda();
    let canonical_bump = Pubkey::find_program_address(&[b"maybe-explicit"], &program_id()).1;

    call(
        &mut svm,
        &payer,
        19,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("first init_if_needed explicit-bump call");
    assert_eq!(
        read_value_and_bump(&svm, &pda),
        Some((1, canonical_bump)),
        "create branch should store the canonical bump for later reuse",
    );

    svm.expire_blockhash();

    call(
        &mut svm,
        &payer,
        19,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("second init_if_needed explicit-bump call");
    assert_eq!(
        read_value_and_bump(&svm, &pda),
        Some((2, canonical_bump)),
        "existing branch should verify against the stored bump and reuse account data",
    );
}

#[test]
fn init_if_needed_rejects_wrong_explicit_bump_on_reuse() {
    let (mut svm, payer, _) = setup();
    let pda = maybe_explicit_bump_pda();

    call(
        &mut svm,
        &payer,
        19,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    )
    .expect("first init_if_needed explicit-bump call");

    let mut account = svm.get_account(&pda).expect("account exists");
    account.data[16] = account.data[16].wrapping_add(1);
    svm.set_account(pda, account).expect("corrupt stored bump");
    svm.expire_blockhash();

    let result = call_raw(
        &mut svm,
        &payer,
        19,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(pda, false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &[],
    );
    assert_err_contains(&result, "InvalidSeeds");
}

// ---- 13. zeroed -----------------------------------------------------------

#[test]
fn zeroed_ok_when_disc_is_zero() {
    let (mut svm, payer, _) = setup();
    let target = Pubkey::new_unique();

    // Pre-create an account owned by the program, sized for Data, with
    // the discriminator bytes set to zero. The derive verifies
    // `data[..8] == [0; 8]`, then stamps the real discriminator and
    // loads mutably.
    // Data = 8 (disc) + 32 (authority) + 8 (value) = 48 bytes.
    let data = vec![0u8; 48];
    let rent = 1_000_000; // well above rent floor for 48 bytes.
    let account = solana_account::Account {
        lamports: rent,
        data,
        owner: program_id(),
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(target, account).expect("set_account");

    call(
        &mut svm,
        &payer,
        13,
        vec![AccountMeta::new(target, false)],
        &[],
    )
    .expect("zeroed disc should pass");
}

#[test]
fn zeroed_rejected_when_disc_non_zero() {
    let (mut svm, payer, _) = setup();
    let target = Pubkey::new_unique();

    // Same shape as above but with a non-zero first byte.
    let mut data = vec![0u8; 48];
    data[0] = 1;
    let account = solana_account::Account {
        lamports: 1_000_000,
        data,
        owner: program_id(),
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(target, account).expect("set_account");

    let result = call_raw(
        &mut svm,
        &payer,
        13,
        vec![AccountMeta::new(target, false)],
        &[],
    );
    assert_custom(&result, CONSTRAINT_ZERO);
}

// ---- 14. signer on UncheckedAccount ---------------------------------------

#[test]
fn signer_on_unchecked_ok_when_signed() {
    let (mut svm, payer, _) = setup();
    let user = keypair_for("signer-user");
    // No airdrop needed — the signer doesn't have to be a funded
    // account, just a valid signature over the tx.
    call(
        &mut svm,
        &payer,
        14,
        vec![AccountMeta::new_readonly(user.pubkey(), true)],
        &[&user],
    )
    .expect("signed");
}

// ---- 17/18. address = <Into<Address> expr> -------------------------------
//
// Asserts the constraint accepts any `Into<Address>` RHS, not just an
// `Address` literal. Two fixtures cover the two non-trivial impls in
// `solana-address`:
//   - `From<[u8; 32]> for Address` — `address = SOME_BYTES`
//   - `From<&Address>  for Address` — `address = some_fn_returning_ref()`

#[test]
fn address_into_from_byte_array_match_ok() {
    let (mut svm, payer, _) = setup();
    call(
        &mut svm,
        &payer,
        17,
        vec![AccountMeta::new_readonly(pinned_address(), false)],
        &[],
    )
    .expect("address from [u8; 32]");
}

#[test]
fn address_into_from_byte_array_mismatch_rejected() {
    let (mut svm, payer, _) = setup();
    let result = call_raw(
        &mut svm,
        &payer,
        17,
        vec![AccountMeta::new_readonly(Pubkey::new_unique(), false)],
        &[],
    );
    assert_err_contains(&result, "InvalidAccountData");
}

#[test]
fn address_into_from_ref_match_ok() {
    let (mut svm, payer, _) = setup();
    call(
        &mut svm,
        &payer,
        18,
        vec![AccountMeta::new_readonly(pinned_address(), false)],
        &[],
    )
    .expect("address from &Address");
}

#[test]
fn address_into_from_ref_mismatch_rejected() {
    let (mut svm, payer, _) = setup();
    let result = call_raw(
        &mut svm,
        &payer,
        18,
        vec![AccountMeta::new_readonly(Pubkey::new_unique(), false)],
        &[],
    );
    assert_err_contains(&result, "InvalidAccountData");
}

// ---- 16. Multiple constraints on a single field --------------------------
//
// `CheckMultipleConstraints` carries three `constraint`s on its `c` field,
// in source order:
//   1. `constraint(a != b @ BadFirst)`
//   2. `constraint = a != c @ BadSecond`
//   3. `constraint(b != c)`                (default ConstraintRaw)
//
// The integration tests assert (a) all three pass on distinct addresses,
// (b) each individual check trips its own error in isolation, and (c)
// when multiple checks would fail, the first one in source order wins —
// proving the emitter preserves the declared evaluation order.

fn call_multi(
    svm: &mut LiteSVM,
    payer: &Keypair,
    a: Pubkey,
    b: Pubkey,
    c: Pubkey,
) -> TransactionResult {
    call_raw(
        svm,
        payer,
        16,
        vec![
            AccountMeta::new_readonly(a, false),
            AccountMeta::new_readonly(b, false),
            AccountMeta::new_readonly(c, false),
        ],
        &[],
    )
}

#[test]
fn multiple_constraints_all_satisfied_ok() {
    let (mut svm, payer, _) = setup();
    call(
        &mut svm,
        &payer,
        16,
        vec![
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
            AccountMeta::new_readonly(Pubkey::new_unique(), false),
        ],
        &[],
    )
    .expect("all three distinct addresses satisfy every constraint");
}

#[test]
fn multiple_constraints_first_failure_surfaces_first_error() {
    // Only the first constraint fails (a == b). Surfaced error must be
    // its custom code.
    let (mut svm, payer, _) = setup();
    let dup = Pubkey::new_unique();
    let result = call_multi(&mut svm, &payer, dup, dup, Pubkey::new_unique());
    assert_custom(&result, ERR_BAD_FIRST);
}

#[test]
fn multiple_constraints_second_failure_surfaces_second_error() {
    // First passes, second fails (a == c). Third would also pass.
    let (mut svm, payer, _) = setup();
    let dup = Pubkey::new_unique();
    let result = call_multi(&mut svm, &payer, dup, Pubkey::new_unique(), dup);
    assert_custom(&result, ERR_BAD_SECOND);
}

#[test]
fn multiple_constraints_third_failure_surfaces_default_error() {
    // First two pass, third fails (b == c). The third entry has no `@
    // err`, so the default `ConstraintRaw` code surfaces.
    let (mut svm, payer, _) = setup();
    let dup = Pubkey::new_unique();
    let result = call_multi(&mut svm, &payer, Pubkey::new_unique(), dup, dup);
    assert_custom(&result, CONSTRAINT_RAW);
}

#[test]
fn multiple_constraints_short_circuit_on_first_failure() {
    // Both the first AND the second checks would fail. The first one
    // declared (a != b) must fire first and short-circuit the rest, so
    // BadFirst — not BadSecond — is what surfaces. This is the test
    // that proves source-order is the evaluation order.
    let (mut svm, payer, _) = setup();
    let same = Pubkey::new_unique();
    // a == b == c — every check fails. The first in source order wins.
    let result = call_multi(&mut svm, &payer, same, same, same);
    assert_custom(&result, ERR_BAD_FIRST);
}

#[test]
fn signer_on_unchecked_rejected_when_not_signed() {
    let (mut svm, payer, _) = setup();
    let user = keypair_for("signer-user");
    // Pass user as non-signer — the `signer` attr must reject.
    let result = call_raw(
        &mut svm,
        &payer,
        14,
        vec![AccountMeta::new_readonly(user.pubkey(), false)],
        &[],
    );
    // `ConstraintSigner` -> `ProgramError::MissingRequiredSignature`.
    assert_err_contains(&result, "MissingRequiredSignature");
}
