//! Dedicated e2e coverage for `anchor-spl::token_interface`.

use {
    anchor_lang::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_program_option::COption,
    solana_program_pack::Pack as _,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token::{
        solana_program::program_pack::Pack,
        state::{Account as SplTokenAccount, Mint as SplMint},
    },
    spl_token_2022_interface::{
        extension::{
            non_transferable::{NonTransferable, NonTransferableAccount},
            set_account_type, BaseStateWithExtensions, BaseStateWithExtensionsMut, ExtensionType,
            StateWithExtensions, StateWithExtensionsMut,
        },
        state::{Account as Token2022Account, Mint as Token2022Mint},
    },
    tests_v2::{build_program, keypair_for, send_instruction},
};

fn program_id() -> Pubkey {
    "79t3uDwfPMnJEgybg7XzsLd54wDyrhskVwhgnmjkRAXj"
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

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/token-interface").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("token_interface_test.so"))
        .expect("load token_interface_test program");

    let payer = keypair_for("token-interface-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn check_token_program(
    svm: &mut LiteSVM,
    payer: &Keypair,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![AccountMeta::new_readonly(token_program, false)];
    send_instruction(svm, program_id(), vec![0], metas, payer, &[])
}

fn init_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    authority: &Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![1], metas, payer, &[mint])
}

fn init_mint_decimals_9(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    authority: &Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![6], metas, payer, &[mint])
}

fn init_mint_with_freeze_authority(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    authority: &Pubkey,
    freeze_authority: &Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new_readonly(*freeze_authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![7], metas, payer, &[mint])
}

fn init_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    token_account: &Keypair,
    authority: &Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(token_account.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![2], metas, payer, &[token_account])
}

fn init_mint_pda(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    authority: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![9], metas, payer, &[])
}

fn init_token_account_pda(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    token_account: Pubkey,
    authority: Pubkey,
    token_program: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(token_account, false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![10], metas, payer, &[])
}

fn check_token_constraints(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    authority: Pubkey,
    token_program: Pubkey,
    token_account: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(token_account, false),
    ];
    send_instruction(svm, program_id(), vec![3], metas, payer, &[])
}

fn check_mint_constraints(
    svm: &mut LiteSVM,
    payer: &Keypair,
    authority: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new_readonly(mint, false),
    ];
    send_instruction(svm, program_id(), vec![4], metas, payer, &[])
}

fn check_mint_freeze_authority(
    svm: &mut LiteSVM,
    payer: &Keypair,
    expected: Pubkey,
    mint: Pubkey,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new_readonly(expected, false),
        AccountMeta::new_readonly(mint, false),
    ];
    send_instruction(svm, program_id(), vec![8], metas, payer, &[])
}

fn mint_to(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    token_account: Pubkey,
    authority: &Keypair,
    token_program: Pubkey,
    amount: u64,
) -> anyhow::Result<litesvm::types::TransactionMetadata> {
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new(token_account, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(token_program, false),
    ];
    let mut data = vec![5];
    data.extend_from_slice(&amount.to_le_bytes());
    send_instruction(svm, program_id(), data, metas, payer, &[authority])
}

fn init_pair(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_authority: &Keypair,
    token_owner: &Keypair,
    token_program: Pubkey,
) -> (Keypair, Keypair) {
    let mint = Keypair::new();
    let token_account = Keypair::new();

    init_mint(svm, payer, &mint, &mint_authority.pubkey(), token_program)
        .expect("interface mint init should succeed");
    init_token_account(
        svm,
        payer,
        &mint.pubkey(),
        &token_account,
        &token_owner.pubkey(),
        token_program,
    )
    .expect("interface token account init should succeed");

    (mint, token_account)
}

fn assert_mint_state(svm: &LiteSVM, mint: Pubkey, owner: Pubkey, expected_authority: Pubkey) {
    let account = svm.get_account(&mint).expect("mint exists");
    assert_eq!(account.owner, owner);
    let state = SplMint::unpack(&account.data).expect("unpack mint");
    assert_eq!(state.decimals, 6);
    assert!(state.is_initialized);
    assert_eq!(
        state.mint_authority.unwrap().to_bytes(),
        expected_authority.to_bytes()
    );
}

fn assert_token_state(
    svm: &LiteSVM,
    token_account: Pubkey,
    owner_program: Pubkey,
    mint: Pubkey,
    authority: Pubkey,
    amount: u64,
) {
    let account = svm
        .get_account(&token_account)
        .expect("token account exists");
    assert_eq!(account.owner, owner_program);
    let state = SplTokenAccount::unpack(&account.data).expect("unpack token account");
    assert_eq!(state.mint.to_bytes(), mint.to_bytes());
    assert_eq!(state.owner.to_bytes(), authority.to_bytes());
    assert_eq!(state.amount, amount);
}

fn seed_initialized_non_transferable_mint(svm: &mut LiteSVM, mint: Pubkey, authority: Pubkey) {
    let len = ExtensionType::try_calculate_account_len::<Token2022Mint>(&[
        ExtensionType::NonTransferable,
    ])
    .expect("calculate token-2022 mint length with non-transferable extension");
    let mut data = vec![0; len];
    {
        let mut state = StateWithExtensionsMut::<Token2022Mint>::unpack_uninitialized(&mut data)
            .expect("unpack uninitialized mint");
        state
            .init_extension::<NonTransferable>(false)
            .expect("initialize non-transferable extension");
    }
    Token2022Mint {
        mint_authority: COption::Some(authority),
        supply: 0,
        decimals: 6,
        is_initialized: true,
        freeze_authority: COption::None,
    }
    .pack_into_slice(&mut data[..Token2022Mint::LEN]);
    set_account_type::<Token2022Mint>(&mut data).expect("set token-2022 mint account type");

    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data,
            owner: token_2022_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed initialized token-2022 mint");
}

fn assert_non_transferable_token_2022_state(
    svm: &LiteSVM,
    token_account: Pubkey,
    mint: Pubkey,
    authority: Pubkey,
) {
    let account = svm
        .get_account(&token_account)
        .expect("token account exists");
    assert_eq!(account.owner, token_2022_program_id());

    let expected_len = ExtensionType::try_calculate_account_len::<Token2022Account>(&[
        ExtensionType::NonTransferableAccount,
        ExtensionType::ImmutableOwner,
    ])
    .expect("calculate token-2022 token account length");
    assert_eq!(account.data.len(), expected_len);

    let state =
        StateWithExtensions::<Token2022Account>::unpack(&account.data).expect("unpack token-2022");
    assert_eq!(state.base.mint.to_bytes(), mint.to_bytes());
    assert_eq!(state.base.owner.to_bytes(), authority.to_bytes());
    assert_eq!(state.base.amount, 0);

    let extensions = state
        .get_extension_types()
        .expect("read token-2022 account extension types");
    assert!(extensions.contains(&ExtensionType::NonTransferableAccount));
    assert!(extensions.contains(&ExtensionType::ImmutableOwner));
    state
        .get_extension::<NonTransferableAccount>()
        .expect("non-transferable account extension should be initialized");
}

#[test]
fn token_interface_program_accepts_token_and_token_2022_only() {
    let (mut svm, payer) = setup();

    check_token_program(&mut svm, &payer, token_program_id()).expect("Token program should pass");
    check_token_program(&mut svm, &payer, token_2022_program_id())
        .expect("Token-2022 program should pass");

    let result = check_token_program(&mut svm, &payer, solana_sdk_ids::system_program::ID);
    assert!(
        result.is_err(),
        "System program must not pass TokenInterface"
    );
}

#[test]
fn interface_init_creates_legacy_mint_and_token_account() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("legacy-interface-mint-authority");
    let token_owner = keypair_for("legacy-interface-token-owner");

    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_program_id(),
    );

    assert_mint_state(
        &svm,
        mint.pubkey(),
        token_program_id(),
        mint_authority.pubkey(),
    );
    assert_token_state(
        &svm,
        token_account.pubkey(),
        token_program_id(),
        mint.pubkey(),
        token_owner.pubkey(),
        0,
    );
}

#[test]
fn interface_init_creates_token_2022_mint_and_token_account() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("t22-interface-mint-authority");
    let token_owner = keypair_for("t22-interface-token-owner");

    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_2022_program_id(),
    );

    assert_mint_state(
        &svm,
        mint.pubkey(),
        token_2022_program_id(),
        mint_authority.pubkey(),
    );
    assert_token_state(
        &svm,
        token_account.pubkey(),
        token_2022_program_id(),
        mint.pubkey(),
        token_owner.pubkey(),
        0,
    );
}

#[test]
fn interface_init_sizes_token_2022_accounts_from_mint_extensions() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("t22-interface-extended-mint-authority");
    let token_owner = keypair_for("t22-interface-extended-token-owner");
    let mint = Keypair::new();
    let token_account = Keypair::new();

    seed_initialized_non_transferable_mint(&mut svm, mint.pubkey(), mint_authority.pubkey());

    init_token_account(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &token_account,
        &token_owner.pubkey(),
        token_2022_program_id(),
    )
    .expect("interface token account init should size from required Token-2022 extensions");

    assert_non_transferable_token_2022_state(
        &svm,
        token_account.pubkey(),
        mint.pubkey(),
        token_owner.pubkey(),
    );
}

#[test]
fn interface_mint_init_supports_program_derived_addresses() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("interface-pda-mint-authority");
    let (mint, _bump) = Pubkey::find_program_address(
        &[b"interface-mint", mint_authority.pubkey().as_ref()],
        &program_id(),
    );

    init_mint_pda(
        &mut svm,
        &payer,
        mint,
        mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("interface mint PDA init should create and initialize a Token-2022 mint");

    assert_mint_state(&svm, mint, token_2022_program_id(), mint_authority.pubkey());
}

#[test]
fn interface_mint_init_rejects_wrong_program_derived_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("interface-pda-mint-reject-authority");
    let wrong_mint = Pubkey::new_unique();

    let result = init_mint_pda(
        &mut svm,
        &payer,
        wrong_mint,
        mint_authority.pubkey(),
        token_2022_program_id(),
    );
    assert!(
        result.is_err(),
        "interface mint PDA init should reject a non-canonical PDA"
    );
    assert!(svm.get_account(&wrong_mint).is_none());
}

#[test]
fn interface_token_account_init_supports_program_derived_addresses() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("interface-token-pda-mint-authority");
    let token_owner = keypair_for("interface-token-pda-owner");
    let mint = Keypair::new();
    init_mint(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed Token-2022 mint");
    let (token_account, _bump) = Pubkey::find_program_address(
        &[
            b"interface-token-account",
            mint.pubkey().as_ref(),
            token_owner.pubkey().as_ref(),
        ],
        &program_id(),
    );

    init_token_account_pda(
        &mut svm,
        &payer,
        mint.pubkey(),
        token_account,
        token_owner.pubkey(),
        token_2022_program_id(),
    )
    .expect("interface token account PDA init should create and initialize a Token-2022 account");

    assert_token_state(
        &svm,
        token_account,
        token_2022_program_id(),
        mint.pubkey(),
        token_owner.pubkey(),
        0,
    );
}

#[test]
fn interface_token_account_init_rejects_wrong_program_derived_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("interface-token-pda-reject-mint-authority");
    let token_owner = keypair_for("interface-token-pda-reject-owner");
    let mint = Keypair::new();
    let wrong_token_account = Pubkey::new_unique();
    init_mint(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed Token-2022 mint");

    let result = init_token_account_pda(
        &mut svm,
        &payer,
        mint.pubkey(),
        wrong_token_account,
        token_owner.pubkey(),
        token_2022_program_id(),
    );
    assert!(
        result.is_err(),
        "interface token account PDA init should reject a non-canonical PDA"
    );
    assert!(svm.get_account(&wrong_token_account).is_none());
}

#[test]
fn interface_init_rejects_non_token_interface_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("bad-interface-mint-authority");
    let token_owner = keypair_for("bad-interface-token-owner");
    let mint = Keypair::new();

    let result = init_mint(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        solana_sdk_ids::system_program::ID,
    );
    assert!(
        result.is_err(),
        "mint init should reject a non-token interface program"
    );
    assert!(svm.get_account(&mint.pubkey()).is_none());

    let mint = Keypair::new();
    let token_account = Keypair::new();
    init_mint(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        token_program_id(),
    )
    .expect("seed mint");

    let result = init_token_account(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &token_account,
        &token_owner.pubkey(),
        solana_sdk_ids::system_program::ID,
    );
    assert!(
        result.is_err(),
        "token account init should reject a non-token interface program"
    );
    assert!(svm.get_account(&token_account.pubkey()).is_none());
}

#[test]
fn interface_constraints_accept_matching_token_2022_accounts() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-t22-mint-authority");
    let token_owner = keypair_for("constraint-t22-token-owner");
    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_2022_program_id(),
    );

    check_mint_constraints(
        &mut svm,
        &payer,
        mint_authority.pubkey(),
        token_2022_program_id(),
        mint.pubkey(),
    )
    .expect("matching mint constraints should pass");
    check_token_constraints(
        &mut svm,
        &payer,
        mint.pubkey(),
        token_owner.pubkey(),
        token_2022_program_id(),
        token_account.pubkey(),
    )
    .expect("matching token constraints should pass");
}

#[test]
fn interface_constraints_reject_wrong_token_program_for_account_owner() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-wrong-program-mint-authority");
    let token_owner = keypair_for("constraint-wrong-program-token-owner");
    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_2022_program_id(),
    );

    let result = check_mint_constraints(
        &mut svm,
        &payer,
        mint_authority.pubkey(),
        token_program_id(),
        mint.pubkey(),
    );
    assert!(
        result.is_err(),
        "Token-2022 mint should reject legacy Token as token_program"
    );

    let result = check_token_constraints(
        &mut svm,
        &payer,
        mint.pubkey(),
        token_owner.pubkey(),
        token_program_id(),
        token_account.pubkey(),
    );
    assert!(
        result.is_err(),
        "Token-2022 token account should reject legacy Token as token_program"
    );
}

#[test]
fn interface_token_constraints_reject_mint_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-wrong-mint-authority");
    let token_owner = keypair_for("constraint-wrong-mint-token-owner");
    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_2022_program_id(),
    );
    let other_mint = Keypair::new();
    init_mint(
        &mut svm,
        &payer,
        &other_mint,
        &mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed mismatched mint");

    let result = check_token_constraints(
        &mut svm,
        &payer,
        other_mint.pubkey(),
        token_owner.pubkey(),
        token_2022_program_id(),
        token_account.pubkey(),
    );
    assert!(
        result.is_err(),
        "token::mint must reject a token account for a different mint"
    );

    assert_token_state(
        &svm,
        token_account.pubkey(),
        token_2022_program_id(),
        mint.pubkey(),
        token_owner.pubkey(),
        0,
    );
}

#[test]
fn interface_token_constraints_reject_authority_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-wrong-owner-mint-authority");
    let token_owner = keypair_for("constraint-wrong-owner-token-owner");
    let wrong_owner = keypair_for("constraint-wrong-owner-other");
    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_2022_program_id(),
    );

    let result = check_token_constraints(
        &mut svm,
        &payer,
        mint.pubkey(),
        wrong_owner.pubkey(),
        token_2022_program_id(),
        token_account.pubkey(),
    );
    assert!(
        result.is_err(),
        "token::authority must reject a token account owned by another authority"
    );
}

#[test]
fn interface_mint_constraints_reject_authority_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-wrong-mint-auth");
    let wrong_authority = keypair_for("constraint-wrong-mint-auth-other");
    let mint = Keypair::new();
    init_mint(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed mint");

    let result = check_mint_constraints(
        &mut svm,
        &payer,
        wrong_authority.pubkey(),
        token_2022_program_id(),
        mint.pubkey(),
    );
    assert!(
        result.is_err(),
        "mint::authority must reject a mint with another authority"
    );
}

#[test]
fn interface_mint_constraints_reject_decimals_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-wrong-decimals-mint-authority");
    let mint = Keypair::new();
    init_mint_decimals_9(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed nine-decimal mint");

    let result = check_mint_constraints(
        &mut svm,
        &payer,
        mint_authority.pubkey(),
        token_2022_program_id(),
        mint.pubkey(),
    );
    assert!(
        result.is_err(),
        "mint::decimals = 6 must reject a nine-decimal mint"
    );
}

#[test]
fn interface_mint_freeze_authority_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-freeze-match-mint-authority");
    let freeze_authority = keypair_for("constraint-freeze-match-freeze-authority");
    let mint = Keypair::new();
    init_mint_with_freeze_authority(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        &freeze_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed mint with freeze authority");

    check_mint_freeze_authority(&mut svm, &payer, freeze_authority.pubkey(), mint.pubkey())
        .expect("matching mint::freeze_authority should pass");
}

#[test]
fn interface_mint_freeze_authority_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-freeze-wrong-mint-authority");
    let freeze_authority = keypair_for("constraint-freeze-wrong-freeze-authority");
    let wrong_freeze_authority = keypair_for("constraint-freeze-wrong-other");
    let mint = Keypair::new();
    init_mint_with_freeze_authority(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        &freeze_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed mint with freeze authority");

    let result = check_mint_freeze_authority(
        &mut svm,
        &payer,
        wrong_freeze_authority.pubkey(),
        mint.pubkey(),
    );
    assert!(
        result.is_err(),
        "mint::freeze_authority must reject a different freeze authority"
    );
}

#[test]
fn interface_mint_freeze_authority_constraint_rejects_unset() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("constraint-freeze-unset-mint-authority");
    let expected_freeze_authority = keypair_for("constraint-freeze-unset-expected");
    let mint = Keypair::new();
    init_mint(
        &mut svm,
        &payer,
        &mint,
        &mint_authority.pubkey(),
        token_2022_program_id(),
    )
    .expect("seed mint without freeze authority");

    let result = check_mint_freeze_authority(
        &mut svm,
        &payer,
        expected_freeze_authority.pubkey(),
        mint.pubkey(),
    );
    assert!(
        result.is_err(),
        "mint::freeze_authority must reject an unset freeze authority"
    );
}

#[test]
fn token_cpi_helpers_work_with_token_interface_accounts() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("cpi-interface-mint-authority");
    let token_owner = keypair_for("cpi-interface-token-owner");
    let (mint, token_account) = init_pair(
        &mut svm,
        &payer,
        &mint_authority,
        &token_owner,
        token_2022_program_id(),
    );

    mint_to(
        &mut svm,
        &payer,
        mint.pubkey(),
        token_account.pubkey(),
        &mint_authority,
        token_2022_program_id(),
        777,
    )
    .expect("mint_to should work through interface accounts");

    assert_token_state(
        &svm,
        token_account.pubkey(),
        token_2022_program_id(),
        mint.pubkey(),
        token_owner.pubkey(),
        777,
    );
}
