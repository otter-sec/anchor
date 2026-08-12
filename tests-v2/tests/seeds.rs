//! Integration tests for PDA seeds in every supported form.
//!
//! Tests array-literal seeds, function-call expression seeds,
//! const-item seeds, mixed (literal + field-ref) seeds, and both
//! bare-bump (runtime find) and explicit stored-bump variants.

use {
    anchor_lang_v2::{
        solana_program::instruction::{AccountMeta, Instruction},
        ToAccountMetas,
    },
    litesvm::{types::TransactionResult, LiteSVM},
    sha2::{Digest, Sha256},
    solana_account::Account,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_pubkey::{bytes_are_curve_point, Pubkey},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "Hyc9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp"
        .parse()
        .unwrap()
}

fn data_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"data"], &program_id()).0
}

fn user_pda(user: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"user", user.as_ref()], &program_id()).0
}

fn raw_program_address(seeds: &[&[u8]], program_id: &Pubkey) -> Pubkey {
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program_id.as_ref());
    hasher.update(b"ProgramDerivedAddress");
    Pubkey::new_from_array(hasher.finalize().into())
}

fn find_on_curve_bump() -> (Pubkey, u8) {
    for bump in 0..=u8::MAX {
        let bump_seed = [bump];
        let address = raw_program_address(&[b"data", &bump_seed], &program_id());
        if bytes_are_curve_point(address.to_bytes()) {
            assert!(
                Pubkey::create_program_address(&[b"data", &bump_seed], &program_id()).is_err(),
                "host PDA derivation should reject on-curve addresses",
            );
            return (address, bump);
        }
    }
    panic!("expected at least one on-curve bump");
}

fn non_canonical_data_pda() -> (Pubkey, u8) {
    let (canonical, canonical_bump) = Pubkey::find_program_address(&[b"data"], &program_id());
    for bump in (0..=u8::MAX).rev() {
        if bump == canonical_bump {
            continue;
        }
        let bump_seed = [bump];
        if let Ok(address) = Pubkey::create_program_address(&[b"data", &bump_seed], &program_id()) {
            assert_ne!(address, canonical);
            return (address, bump);
        }
    }
    panic!("expected at least one non-canonical off-curve bump");
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/seeds").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("seeds.so"))
        .expect("deploy");

    let payer = keypair_for("payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

/// Helper: call an ix by discriminator with the given accounts.
fn call(
    svm: &mut LiteSVM,
    payer: &Keypair,
    disc: u8,
    accounts: Vec<AccountMeta>,
) -> anyhow::Result<()> {
    send_instruction(svm, program_id(), vec![disc], accounts, payer, &[])?;
    Ok(())
}

// ---- Tests -----------------------------------------------------------------

#[test]
fn init_and_check_literal_seeds() {
    let (mut svm, payer) = setup();
    let data = data_pda();

    // 1. Init with literal seeds
    call(
        &mut svm,
        &payer,
        0,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_literal");

    // 2. Check literal seeds + bare bump
    call(
        &mut svm,
        &payer,
        1,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    )
    .expect("check_literal");

    // 3. Check literal seeds + explicit bump
    call(
        &mut svm,
        &payer,
        2,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    )
    .expect("check_literal_explicit_bump");
}

#[test]
fn check_fn_seeds() {
    let (mut svm, payer) = setup();
    let data = data_pda();

    // Init first
    call(
        &mut svm,
        &payer,
        0,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_literal");

    // 4. Function-call seeds + bare bump
    call(
        &mut svm,
        &payer,
        3,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    )
    .expect("check_fn_seeds");

    // 5. Function-call seeds + explicit bump
    call(
        &mut svm,
        &payer,
        4,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    )
    .expect("check_fn_seeds_explicit_bump");
}

#[test]
fn check_const_seeds() {
    let (mut svm, payer) = setup();
    let data = data_pda();

    // Init first
    call(
        &mut svm,
        &payer,
        0,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_literal");

    // 6. Const-item seeds + bare bump
    call(
        &mut svm,
        &payer,
        5,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    )
    .expect("check_const_seeds");
}

#[test]
fn init_and_check_mixed_seeds() {
    let (mut svm, payer) = setup();
    let data = user_pda(&payer.pubkey());

    // 7. Init with mixed seeds
    call(
        &mut svm,
        &payer,
        7,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_mixed");

    // 8. Check mixed seeds
    call(
        &mut svm,
        &payer,
        8,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    )
    .expect("check_mixed");
}

#[test]
fn wrong_pda_rejected() {
    let (mut svm, payer) = setup();
    let data = data_pda();

    // Init first
    call(
        &mut svm,
        &payer,
        0,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_literal");

    // Pass wrong account for fn-seeds check — should fail
    let wrong = Pubkey::new_unique();
    let result = call(
        &mut svm,
        &payer,
        3,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(wrong, false),
        ],
    );
    assert!(result.is_err(), "wrong PDA should be rejected");
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

fn set_system_account(svm: &mut LiteSVM, address: Pubkey, lamports: u64, data_len: usize) {
    svm.set_account(
        address,
        Account {
            lamports,
            data: vec![0u8; data_len],
            owner: solana_sdk_ids::system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

#[test]
fn wrong_bump_value_rejected() {
    let (mut svm, payer) = setup();
    let data = data_pda();

    // Init the PDA — stores canonical bump in data.bump (byte offset 16)
    call(
        &mut svm,
        &payer,
        0,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_literal");

    // Read the canonical bump and corrupt it
    let mut account = svm.get_account(&data).expect("data exists");
    let original_bump = account.data[16];
    account.data[16] = original_bump.wrapping_add(1); // off-by-one
    svm.set_account(data, account).unwrap();

    // check_literal_explicit_bump (discrim=2) uses `bump = data.bump`
    // — the corrupted bump won't match the PDA derivation
    let result = call_raw(
        &mut svm,
        &payer,
        2,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(data, false),
        ],
    );
    assert!(result.is_err(), "off-by-one bump should be rejected");
    let err = format!("{:?}", result.unwrap_err().err);
    assert!(
        err.contains("InvalidSeeds") || err.contains("Custom("),
        "expected InvalidSeeds error, got: {err}"
    );
}

#[test]
fn mixed_seeds_wrong_order_rejected() {
    let (mut svm, payer) = setup();

    // Derive PDA with reversed seed order: [payer, b"user"] instead of [b"user", payer]
    let wrong_pda =
        Pubkey::find_program_address(&[payer.pubkey().as_ref(), b"user"], &program_id()).0;

    // Init the correct PDA first so the program is deployed
    let correct_pda = user_pda(&payer.pubkey());
    call(
        &mut svm,
        &payer,
        7,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(correct_pda, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
    )
    .expect("init_mixed");

    svm.expire_blockhash();

    // check_mixed (discrim=8) with the reversed-seed PDA
    let result = call_raw(
        &mut svm,
        &payer,
        8,
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(wrong_pda, false),
        ],
    );
    assert!(
        result.is_err(),
        "PDA from wrong seed order should be rejected"
    );
}

#[test]
fn literal_untrusted_bump_rejects_on_curve_address() {
    let (mut svm, payer) = setup();
    let (fake_pda, bump) = find_on_curve_bump();
    set_system_account(&mut svm, fake_pda, 1_000_000, 0);

    let result = send_instruction(
        &mut svm,
        program_id(),
        vec![12, bump],
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(fake_pda, false),
        ],
        &payer,
        &[],
    );
    assert!(
        result.is_err(),
        "on-curve address derived from explicit bump must be rejected",
    );
}

// `bump = <expr>` must not auto-derive via find_program_address on Resolved.
#[test]
fn resolved_builder_requires_address_for_explicit_bump() {
    let (mut svm, payer) = setup();
    let (pda, bump) = non_canonical_data_pda();
    assert_ne!(pda, data_pda(), "fixture must differ from the canonical PDA");

    // Compiles only if `data` is Required (not Pda-auto-derived).
    let metas = seeds::accounts::CheckLiteralUntrustedBumpResolved {
        payer: payer.pubkey(),
        data: pda,
    }
    .to_account_metas(None);
    assert_eq!(metas[1].pubkey, pda);

    set_system_account(&mut svm, pda, 1_000_000, 0);
    send_instruction(
        &mut svm,
        program_id(),
        vec![12, bump],
        metas,
        &payer,
        &[],
    )
    .expect("non-canonical explicit-bump PDA accepted when address is supplied");
}

#[test]
fn fn_seed_untrusted_bump_rejects_on_curve_address() {
    let (mut svm, payer) = setup();
    let (fake_pda, bump) = find_on_curve_bump();
    set_system_account(&mut svm, fake_pda, 1_000_000, 0);

    let result = send_instruction(
        &mut svm,
        program_id(),
        vec![13, bump],
        vec![
            AccountMeta::new_readonly(payer.pubkey(), true),
            AccountMeta::new_readonly(fake_pda, false),
        ],
        &payer,
        &[],
    );
    assert!(
        result.is_err(),
        "on-curve address derived from explicit bump must be rejected",
    );
}
