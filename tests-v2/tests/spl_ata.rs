//! Focused tests for `associated_token::*` account constraints.
//!
//! Most cases are negative: the happy path exists only to create state that
//! the validation failures can mutate around.

use {
    anchor_lang_v2::solana_program::instruction::AccountMeta,
    anchor_lang_v2::ToAccountMetas,
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token::{solana_program::program_pack::Pack, state::Account as SplTokenAccount},
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "AtA1111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn token_program_id() -> Pubkey {
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        .parse()
        .unwrap()
}

fn token_2022_program_id() -> Pubkey {
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        .parse()
        .unwrap()
}

fn ata_program_id() -> Pubkey {
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        .parse()
        .unwrap()
}

fn associated_token_address(owner: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program_id(),
    )
    .0
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/spl-ata").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("spl_ata_test.so"))
        .expect("load spl_ata_test program");

    let payer = keypair_for("spl-ata-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn init_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, authority: &Pubkey) {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![0], metas, payer, &[mint])
        .expect("init_mint should succeed");
}

fn init_interface_mint_with_token_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    authority: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![9], metas, payer, &[mint]).map(|_| ())
}

fn init_ata_ix(mint: Pubkey, owner: Pubkey, ata: Pubkey) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ]
}

fn send_init_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
) -> anyhow::Result<()> {
    send_init_ata_with_programs(
        svm,
        payer,
        mint,
        owner,
        ata,
        token_program_id(),
        ata_program_id(),
    )
}

fn send_init_ata_with_programs(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
    ata_program: Pubkey,
) -> anyhow::Result<()> {
    send_init_ata_with_programs_and_system(
        svm,
        payer,
        mint,
        owner,
        ata,
        token_program,
        ata_program,
        solana_sdk_ids::system_program::ID,
    )
}

fn send_init_ata_with_programs_and_system(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
    ata_program: Pubkey,
    system_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(ata_program, false),
        AccountMeta::new_readonly(system_program, false),
    ];
    send_instruction(svm, program_id(), vec![2], metas, payer, &[]).map(|_| ())
}

fn send_direct_create_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
    ata_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(ata_program, false),
    ];
    send_instruction(svm, program_id(), vec![13], metas, payer, &[]).map(|_| ())
}

fn send_init_interface_ata_with_token_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(ata_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![6], metas, payer, &[]).map(|_| ())
}

fn send_init_strict_ata_with_token_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(ata_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![14], metas, payer, &[]).map(|_| ())
}

fn send_init_ata_if_needed(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(ata_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![3], metas, payer, &[]).map(|_| ())
}

fn send_validate_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
) -> anyhow::Result<()> {
    send_instruction(
        svm,
        program_id(),
        vec![4],
        init_ata_ix(mint, owner, ata),
        payer,
        &[],
    )
    .map(|_| ())
}

fn send_validate_interface_ata_with_token_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program, false),
    ];
    send_instruction(svm, program_id(), vec![7], metas, payer, &[]).map(|_| ())
}

fn send_init_interface_ata_if_needed_with_token_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    ata: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(ata_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![10], metas, payer, &[]).map(|_| ())
}

fn send_init_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    owner: Pubkey,
    token_account: &Keypair,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new(token_account.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![1], metas, payer, &[token_account]).map(|_| ())
}

fn send_mint_to_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    ata: Pubkey,
    owner: Pubkey,
    authority: &Keypair,
    amount: u64,
) -> anyhow::Result<()> {
    let mut data = vec![8];
    data.extend_from_slice(&amount.to_le_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(owner, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[authority]).map(|_| ())
}

fn send_set_ata_owner(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ata: Pubkey,
    current_authority: &Keypair,
    new_authority: Pubkey,
) -> anyhow::Result<()> {
    let metas = vec![
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(current_authority.pubkey(), true),
        AccountMeta::new_readonly(new_authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(
        svm,
        program_id(),
        vec![12],
        metas,
        payer,
        &[current_authority],
    )
    .map(|_| ())
}

fn mint_and_ata(svm: &mut LiteSVM, payer: &Keypair) -> (Keypair, Keypair, Pubkey) {
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_program_id());
    init_mint(svm, payer, &mint, &mint_authority.pubkey());
    send_init_ata(svm, payer, mint.pubkey(), owner.pubkey(), ata).expect("init ata");
    (mint, owner, ata)
}

fn assert_token_account_state(
    svm: &LiteSVM,
    token_account: Pubkey,
    expected_mint: Pubkey,
    expected_owner: Pubkey,
    expected_program: Pubkey,
) {
    let account = svm
        .get_account(&token_account)
        .expect("token account exists");
    assert!(account.data.len() >= SplTokenAccount::LEN);
    let state = SplTokenAccount::unpack_from_slice(&account.data[..SplTokenAccount::LEN])
        .expect("unpack token account");
    assert_eq!(state.mint.to_bytes(), expected_mint.to_bytes());
    assert_eq!(state.owner.to_bytes(), expected_owner.to_bytes());
    assert_eq!(state.amount, 0);
    assert_eq!(account.owner, expected_program);
}

#[test]
fn init_rejects_non_canonical_ata_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let wrong_ata = Keypair::new().pubkey();
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_init_ata(&mut svm, &payer, mint.pubkey(), owner.pubkey(), wrong_ata).is_err());
    assert!(svm.get_account(&wrong_ata).is_none());
}

#[test]
fn init_rejects_wrong_associated_token_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_program_id());
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_init_ata_with_programs(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_program_id(),
        token_program_id(),
    )
    .is_err());
    assert!(svm.get_account(&ata).is_none());
}

#[test]
fn direct_create_rejects_wrong_associated_token_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_program_id());
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_direct_create_ata(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_program_id(),
        token_program_id(),
    )
    .is_err());
    assert!(svm.get_account(&ata).is_none());
}

#[test]
fn init_rejects_wrong_system_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_program_id());
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_init_ata_with_programs_and_system(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_program_id(),
        ata_program_id(),
        token_program_id(),
    )
    .is_err());
    assert!(svm.get_account(&ata).is_none());
}

#[test]
fn validate_rejects_wrong_ata_address_even_when_token_state_matches() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let non_ata = Keypair::new();
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    send_init_token_account(&mut svm, &payer, mint.pubkey(), owner.pubkey(), &non_ata)
        .expect("plain token account init");

    assert!(send_validate_ata(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        non_ata.pubkey()
    )
    .is_err());
}

#[test]
fn validate_rejects_wrong_mint() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let other_mint = Keypair::new();
    init_mint(&mut svm, &payer, &other_mint, &owner.pubkey());

    assert!(send_validate_ata(&mut svm, &payer, other_mint.pubkey(), owner.pubkey(), ata).is_err());
    assert!(send_validate_ata(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_ok());
}

#[test]
fn validate_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let wrong_owner = keypair_for("spl-ata-wrong-owner");

    assert!(send_validate_ata(&mut svm, &payer, mint.pubkey(), wrong_owner.pubkey(), ata).is_err());
    assert!(send_validate_ata(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_ok());
}

#[test]
fn validate_rejects_ata_after_token_owner_change() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let new_owner = keypair_for("spl-ata-new-owner");

    send_set_ata_owner(&mut svm, &payer, ata, &owner, new_owner.pubkey())
        .expect("change token owner");

    assert!(send_validate_ata(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_err());
    assert!(send_validate_ata(&mut svm, &payer, mint.pubkey(), new_owner.pubkey(), ata).is_err());
}

#[test]
fn validate_with_token_program_rejects_wrong_token_program() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let fake_token_program = Keypair::new().pubkey();

    assert!(send_validate_interface_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        fake_token_program,
    )
    .is_err());
    assert!(send_validate_interface_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_program_id(),
    )
    .is_ok());
}

#[test]
fn init_with_token_program_creates_token_2022_ata() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-token-2022-mint-authority");
    let owner = keypair_for("spl-ata-token-2022-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_2022_program_id());

    init_interface_mint_with_token_program(
        &mut svm,
        &payer,
        &mint,
        mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("init token-2022 mint");
    send_init_interface_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_2022_program_id(),
    )
    .expect("init token-2022 ata");

    assert_token_account_state(
        &svm,
        ata,
        mint.pubkey(),
        owner.pubkey(),
        token_2022_program_id(),
    );
    assert!(send_validate_interface_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_2022_program_id(),
    )
    .is_ok());
}

#[test]
fn strict_ata_init_rejects_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-strict-token-2022-mint-authority");
    let owner = keypair_for("spl-ata-strict-token-2022-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_2022_program_id());

    init_interface_mint_with_token_program(
        &mut svm,
        &payer,
        &mint,
        mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("init token-2022 mint");

    assert!(send_init_strict_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_2022_program_id(),
    )
    .is_err());
    assert!(svm.get_account(&ata).is_none());
}

#[test]
fn init_with_token_program_rejects_wrong_token_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-bad-token-program-mint-authority");
    let owner = keypair_for("spl-ata-bad-token-program-owner");
    let mint = Keypair::new();
    let fake_token_program = Keypair::new().pubkey();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &fake_token_program);
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_init_interface_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        fake_token_program,
    )
    .is_err());
    assert!(svm.get_account(&ata).is_none());
}

#[test]
fn init_if_needed_creates_legacy_ata_when_missing() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_program_id());
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_init_ata_if_needed(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_ok());
    assert_token_account_state(&svm, ata, mint.pubkey(), owner.pubkey(), token_program_id());
}

#[test]
fn init_if_needed_with_token_program_creates_token_2022_ata_when_missing() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-token-2022-mint-authority");
    let owner = keypair_for("spl-ata-token-2022-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_2022_program_id());

    init_interface_mint_with_token_program(
        &mut svm,
        &payer,
        &mint,
        mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("init token-2022 mint");

    assert!(send_init_interface_ata_if_needed_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_2022_program_id(),
    )
    .is_ok());
    assert_token_account_state(
        &svm,
        ata,
        mint.pubkey(),
        owner.pubkey(),
        token_2022_program_id(),
    );
}

#[test]
fn init_if_needed_with_token_program_rejects_missing_wrong_token_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-if-needed-bad-token-program-mint-authority");
    let owner = keypair_for("spl-ata-if-needed-bad-token-program-owner");
    let mint = Keypair::new();
    let fake_token_program = Keypair::new().pubkey();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &fake_token_program);
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    assert!(send_init_interface_ata_if_needed_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        fake_token_program,
    )
    .is_err());
    assert!(svm.get_account(&ata).is_none());
}

#[test]
fn init_if_needed_rejects_existing_wrong_mint() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let other_mint = Keypair::new();
    init_mint(&mut svm, &payer, &other_mint, &owner.pubkey());

    assert!(
        send_init_ata_if_needed(&mut svm, &payer, other_mint.pubkey(), owner.pubkey(), ata)
            .is_err()
    );
    assert!(send_init_ata_if_needed(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_ok());
}

#[test]
fn init_if_needed_passes_when_legacy_ata_exists() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);

    assert!(send_init_ata_if_needed(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_ok());
    assert_token_account_state(&svm, ata, mint.pubkey(), owner.pubkey(), token_program_id());
}

#[test]
fn init_if_needed_rejects_existing_non_ata_even_when_token_state_matches() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let non_ata = Keypair::new();
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());
    send_init_token_account(&mut svm, &payer, mint.pubkey(), owner.pubkey(), &non_ata)
        .expect("plain token account init");

    assert!(send_init_ata_if_needed(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        non_ata.pubkey()
    )
    .is_err());
}

#[test]
fn init_if_needed_rejects_existing_wrong_authority() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let wrong_owner = keypair_for("spl-ata-wrong-owner");

    assert!(
        send_init_ata_if_needed(&mut svm, &payer, mint.pubkey(), wrong_owner.pubkey(), ata)
            .is_err()
    );
    assert!(send_init_ata_if_needed(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).is_ok());
}

#[test]
fn init_if_needed_with_token_program_rejects_existing_wrong_token_program() {
    let (mut svm, payer) = setup();
    let (mint, owner, ata) = mint_and_ata(&mut svm, &payer);
    let fake_token_program = Keypair::new().pubkey();

    assert!(send_init_interface_ata_if_needed_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        fake_token_program,
    )
    .is_err());
    assert!(send_init_interface_ata_if_needed_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_program_id(),
    )
    .is_ok());
}

#[test]
fn init_if_needed_with_token_program_passes_when_token_2022_ata_exists() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-token-2022-mint-authority");
    let owner = keypair_for("spl-ata-token-2022-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_2022_program_id());

    init_interface_mint_with_token_program(
        &mut svm,
        &payer,
        &mint,
        mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("init token-2022 mint");
    send_init_interface_ata_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_2022_program_id(),
    )
    .expect("init token-2022 ata");

    assert!(send_init_interface_ata_if_needed_with_token_program(
        &mut svm,
        &payer,
        mint.pubkey(),
        owner.pubkey(),
        ata,
        token_2022_program_id(),
    )
    .is_ok());
    assert_token_account_state(
        &svm,
        ata,
        mint.pubkey(),
        owner.pubkey(),
        token_2022_program_id(),
    );
}

#[test]
fn mut_constraint_still_rejects_wrong_ata_before_cpi() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("spl-ata-mint-authority");
    let owner = keypair_for("spl-ata-owner");
    let mint = Keypair::new();
    let ata = associated_token_address(&owner.pubkey(), &mint.pubkey(), &token_program_id());
    init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());
    send_init_ata(&mut svm, &payer, mint.pubkey(), owner.pubkey(), ata).expect("init ata");

    let wrong_owner = keypair_for("spl-ata-wrong-owner");
    assert!(send_mint_to_ata(
        &mut svm,
        &payer,
        mint.pubkey(),
        ata,
        wrong_owner.pubkey(),
        &mint_authority,
        10,
    )
    .is_err());

    assert!(send_mint_to_ata(
        &mut svm,
        &payer,
        mint.pubkey(),
        ata,
        owner.pubkey(),
        &mint_authority,
        10,
    )
    .is_ok());

    let account = svm.get_account(&ata).expect("ata exists");
    let state = SplTokenAccount::unpack(&account.data).expect("unpack token");
    assert_eq!(state.amount, 10);
}

#[test]
fn created_ata_is_not_marked_signer_in_client_metas() {
    let mint = Keypair::new().pubkey();
    let owner = keypair_for("spl-ata-owner").pubkey();
    let ata = associated_token_address(&owner, &mint, &token_program_id());
    let metas = spl_ata_test::accounts::InitAta {
        payer: keypair_for("spl-ata-payer").pubkey(),
        mint,
        authority: owner,
        token_account: ata,
        token_program: token_program_id(),
        associated_token_program: ata_program_id(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    let token_meta = metas
        .iter()
        .find(|meta| meta.pubkey == ata)
        .expect("ata meta present");
    assert!(token_meta.is_writable);
    assert!(!token_meta.is_signer);
}

#[test]
fn explicit_token_program_accounts_accept_unchecked_program_metas() {
    let mint = Keypair::new().pubkey();
    let owner = keypair_for("spl-ata-owner").pubkey();
    let ata = associated_token_address(&owner, &mint, &token_2022_program_id());
    let metas = spl_ata_test::accounts::InitInterfaceAtaWithTokenProgram {
        payer: keypair_for("spl-ata-payer").pubkey(),
        mint,
        authority: owner,
        token_account: ata,
        token_program: token_2022_program_id(),
        associated_token_program: ata_program_id(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    assert!(metas
        .iter()
        .any(|meta| meta.pubkey == token_2022_program_id() && !meta.is_signer));
    let ata_meta = metas
        .iter()
        .find(|meta| meta.pubkey == ata)
        .expect("ata meta present");
    assert!(ata_meta.is_writable);
    assert!(!ata_meta.is_signer);
}

#[test]
fn many_associated_token_accounts_are_not_marked_signers() {
    let payer = keypair_for("spl-ata-payer").pubkey();
    let mint = Keypair::new().pubkey();
    let atas = [
        associated_token_address(&payer, &mint, &token_program_id()),
        associated_token_address(
            &solana_sdk_ids::system_program::ID,
            &mint,
            &token_program_id(),
        ),
        associated_token_address(&token_program_id(), &mint, &token_program_id()),
        associated_token_address(&ata_program_id(), &mint, &token_program_id()),
        associated_token_address(&mint, &mint, &token_program_id()),
    ];
    let metas = spl_ata_test::accounts::InitManyAssociatedTokenAccounts {
        payer,
        mint,
        payer_ata: atas[0],
        system_ata: atas[1],
        token_program_ata: atas[2],
        associated_token_program_ata: atas[3],
        mint_ata: atas[4],
        token_program: token_program_id(),
        associated_token_program: ata_program_id(),
        system_program: solana_sdk_ids::system_program::ID,
    }
    .to_account_metas(None);

    for ata in atas {
        let meta = metas
            .iter()
            .find(|meta| meta.pubkey == ata)
            .expect("ata meta present");
        assert!(meta.is_writable);
        assert!(!meta.is_signer);
    }
}

#[test]
fn init_many_associated_token_accounts_creates_all_accounts() {
    let (mut svm, payer) = setup();
    let mint = Keypair::new();
    let authorities = [
        payer.pubkey(),
        solana_sdk_ids::system_program::ID,
        token_program_id(),
        ata_program_id(),
        mint.pubkey(),
    ];
    let atas = authorities
        .map(|authority| associated_token_address(&authority, &mint.pubkey(), &token_program_id()));
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new(atas[0], false),
        AccountMeta::new(atas[1], false),
        AccountMeta::new(atas[2], false),
        AccountMeta::new(atas[3], false),
        AccountMeta::new(atas[4], false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(ata_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];

    send_instruction(&mut svm, program_id(), vec![11], metas, &payer, &[&mint])
        .expect("init many atas");

    for (ata, authority) in atas.into_iter().zip(authorities) {
        assert_token_account_state(&svm, ata, mint.pubkey(), authority, token_program_id());
    }
}
