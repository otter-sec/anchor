use {
    anchor_lang_v2::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

const FUNDED_PDA_LAMPORTS: u64 = 1_000_000_000;

fn program_id() -> Pubkey {
    "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
        .parse()
        .unwrap()
}

fn payer_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"payer"], &program_id()).0
}

fn target_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"target"], &program_id()).0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/pda-payer").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("pda_payer_test.so"))
        .expect("deploy pda_payer_test");

    let payer = keypair_for("pda-payer-wallet");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    seed_system_account(&mut svm, payer_pda(), FUNDED_PDA_LAMPORTS, 0);
    (svm, payer)
}

fn seed_system_account(svm: &mut LiteSVM, address: Pubkey, lamports: u64, data_len: usize) {
    svm.set_account(
        address,
        solana_account::Account {
            lamports,
            data: vec![0u8; data_len],
            owner: solana_sdk_ids::system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn account_value(svm: &LiteSVM, address: &Pubkey) -> u64 {
    let account = svm.get_account(address).expect("account exists");
    u64::from_le_bytes(account.data[8..16].try_into().unwrap())
}

#[track_caller]
fn assert_error_contains(err: &anyhow::Error, needle: &str) {
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains(needle),
        "expected error containing {needle:?}, got: {rendered}",
    );
}

#[test]
fn seeded_system_account_can_pay_for_fresh_target() {
    let (mut svm, payer) = setup();
    let funded_pda = payer_pda();
    let new_account = Keypair::new();

    let metas = vec![
        AccountMeta::new(funded_pda, false),
        AccountMeta::new(new_account.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![0],
        metas,
        &payer,
        &[&new_account],
    )
    .expect("fresh-target init should succeed");

    let created = svm
        .get_account(&new_account.pubkey())
        .expect("fresh target account exists");
    let payer_after = svm.get_account(&funded_pda).expect("payer PDA exists");

    assert_eq!(created.owner, program_id());
    assert_eq!(account_value(&svm, &new_account.pubkey()), 42);
    assert!(
        payer_after.lamports < FUNDED_PDA_LAMPORTS,
        "payer PDA should fund the created account"
    );
    assert_eq!(
        payer_after.lamports + created.lamports,
        FUNDED_PDA_LAMPORTS,
        "rent should move from the PDA payer into the new account"
    );
}

#[test]
fn seeded_system_account_can_pay_for_pda_target() {
    let (mut svm, payer) = setup();
    let funded_pda = payer_pda();
    let target = target_pda();

    let metas = vec![
        AccountMeta::new(funded_pda, false),
        AccountMeta::new(target, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![1], metas, &payer, &[])
        .expect("pda-target init should succeed");

    let created = svm.get_account(&target).expect("target PDA exists");
    let payer_after = svm.get_account(&funded_pda).expect("payer PDA exists");

    assert_eq!(created.owner, program_id());
    assert_eq!(account_value(&svm, &target), 7);
    assert!(
        payer_after.lamports < FUNDED_PDA_LAMPORTS,
        "payer PDA should fund the PDA target"
    );
    assert_eq!(
        payer_after.lamports + created.lamports,
        FUNDED_PDA_LAMPORTS,
        "rent should move from the PDA payer into the PDA target"
    );
}

#[test]
fn seeded_system_account_can_pay_for_boxed_target() {
    let (mut svm, payer) = setup();
    let funded_pda = payer_pda();
    let new_account = Keypair::new();

    let metas = vec![
        AccountMeta::new(funded_pda, false),
        AccountMeta::new(new_account.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![2],
        metas,
        &payer,
        &[&new_account],
    )
    .expect("boxed-target init should succeed");

    let created = svm
        .get_account(&new_account.pubkey())
        .expect("boxed target account exists");
    let payer_after = svm.get_account(&funded_pda).expect("payer PDA exists");

    assert_eq!(created.owner, program_id());
    assert_eq!(account_value(&svm, &new_account.pubkey()), 99);
    assert!(
        payer_after.lamports < FUNDED_PDA_LAMPORTS,
        "payer PDA should fund the boxed target"
    );
    assert_eq!(
        payer_after.lamports + created.lamports,
        FUNDED_PDA_LAMPORTS,
        "rent should move from the PDA payer into the boxed target"
    );
}

#[test]
fn payer_with_more_than_max_seeds_is_rejected() {
    let (mut svm, payer) = setup();
    let candidate_payer = payer_pda();
    let new_account = Keypair::new();
    let payer_before = svm
        .get_account(&candidate_payer)
        .expect("candidate payer exists")
        .lamports;

    let metas = vec![
        AccountMeta::new(candidate_payer, false),
        AccountMeta::new(new_account.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    let err = send_instruction(
        &mut svm,
        program_id(),
        vec![3],
        metas,
        &payer,
        &[&new_account],
    )
    .expect_err("payer with more than 16 seeds must fail");

    assert_error_contains(&err, "InvalidSeeds");
    assert!(
        svm.get_account(&new_account.pubkey()).is_none(),
        "failed init must not create the target account"
    );
    assert_eq!(
        svm.get_account(&candidate_payer)
            .expect("candidate payer still exists")
            .lamports,
        payer_before,
        "failed init must not debit the candidate payer"
    );
}

#[test]
fn seeded_system_account_can_pay_for_target_with_opaque_payer_seeds() {
    let (mut svm, payer) = setup();
    let funded_pda = payer_pda();
    let new_account = Keypair::new();

    let metas = vec![
        AccountMeta::new(funded_pda, false),
        AccountMeta::new(new_account.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![4],
        metas,
        &payer,
        &[&new_account],
    )
    .expect("opaque-payer-seeds init should succeed");

    let created = svm
        .get_account(&new_account.pubkey())
        .expect("opaque-seeds target account exists");
    let payer_after = svm.get_account(&funded_pda).expect("payer PDA exists");

    assert_eq!(created.owner, program_id());
    assert_eq!(account_value(&svm, &new_account.pubkey()), 123);
    assert!(
        payer_after.lamports < FUNDED_PDA_LAMPORTS,
        "payer PDA should fund the opaque-seeds target"
    );
    assert_eq!(
        payer_after.lamports + created.lamports,
        FUNDED_PDA_LAMPORTS,
        "rent should move from the PDA payer into the opaque-seeds target"
    );
}
