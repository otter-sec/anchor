use {
    anchor_lang::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "CrVfxA2g7VqBvkYQG4eCyz8YdVCrbsnY6SQWL6gNw7h5"
        .parse()
        .unwrap()
}

fn token_program_id() -> Pubkey {
    <anchor_spl::token::Token as anchor_lang::Id>::id()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir
            .join("programs/constraint-values")
            .to_str()
            .unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("constraint_values.so"))
        .expect("load constraint_values program");
    let payer = keypair_for("constraint-values-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn init_mint(svm: &mut LiteSVM, payer: &Keypair, authority: &Keypair, mint: &Keypair) {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![0], metas, payer, &[authority, mint])
        .expect("init_mint should succeed");
}

#[test]
fn optional_authority_none_returns_constraint_account_is_none() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("constraint-values-authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let mint = Keypair::new();
    init_mint(&mut svm, &payer, &authority, &mint);

    let err = send_instruction(
        &mut svm,
        program_id(),
        vec![1],
        vec![
            AccountMeta::new_readonly(program_id(), false),
            AccountMeta::new(mint.pubkey(), false),
        ],
        &payer,
        &[],
    )
    .unwrap_err()
    .to_string();

    assert!(
        err.contains("Custom(2020)") || err.contains("ConstraintAccountIsNone"),
        "optional None should raise ConstraintAccountIsNone, got: {err}"
    );
}

#[test]
fn update_only_mint_decimals_does_not_leak_into_init_params() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("constraint-values-update-authority");
    svm.airdrop(&authority.pubkey(), 1_000_000_000).unwrap();
    let mint = Keypair::new();

    let err = send_instruction(
        &mut svm,
        program_id(),
        vec![2],
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new(mint.pubkey(), true),
            AccountMeta::new_readonly(token_program_id(), false),
            AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        ],
        &payer,
        &[&authority, &mint],
    )
    .unwrap_err()
    .to_string();

    assert!(
        err.contains("InvalidArgument"),
        "update-only mint::decimals should no longer satisfy init params, got: {err}"
    );
}
