//! Integration tests for `anchor-spl`'s Mint/TokenAccount surface.
//!
//! Exercises the full spl-v2 API:
//!   - Init paths: `#[account(init, mint::*)]` and `#[account(init, token::*)]`
//!   - CPI helpers: mint_to, transfer, transfer_checked, burn, approve,
//!     revoke, close_account
//!   - Accessor methods on Mint and TokenAccount
//!   - Namespaced constraints: mint::decimals, mint::authority,
//!     token::mint, token::authority
//!   - `get_associated_token_address` derivation

use {
    anchor_lang::solana_program::instruction::AccountMeta,
    litesvm::LiteSVM,
    solana_account::Account,
    solana_keypair::Keypair,
    solana_program_option::COption as Token2022COption,
    solana_program_pack::Pack as Token2022Pack,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    spl_token::{
        solana_program::{
            program_option::COption, program_pack::Pack, pubkey::Pubkey as SplPubkey,
        },
        state::{Account as SplTokenAccount, AccountState, Mint as SplMint},
    },
    spl_token_2022_interface::{
        extension::{
            memo_transfer::MemoTransfer, set_account_type, BaseStateWithExtensionsMut,
            ExtensionType, StateWithExtensionsMut,
        },
        state::{
            Account as Token2022Account, AccountState as Token2022AccountState,
            Mint as Token2022Mint,
        },
    },
    std::path::PathBuf,
    tests_v2::{build_program, keypair_for, send_instruction},
};

const TOKEN_2022_MAINNET_SO: &str = "fixtures/programs/token_2022_mainnet.so";

fn program_id() -> Pubkey {
    "SpL1111111111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn wrong_token_2022_program_id() -> Pubkey {
    "7TdHZyhueZP4B8fvbgvbGPTH4bijkBPtpWc3wBfTmWQv"
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

fn token_cpi_ext_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"token-cpi-ext-authority"], &program_id())
}

fn native_mint_id() -> Pubkey {
    "So11111111111111111111111111111111111111112"
        .parse()
        .unwrap()
}

fn token_2022_native_mint_id() -> Pubkey {
    anchor_spl::token_2022::native_mint::id()
}

fn ata_program_id() -> Pubkey {
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        .parse()
        .unwrap()
}

fn token_2022_mainnet_so_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TOKEN_2022_MAINNET_SO)
}

fn rent_sysvar_id() -> Pubkey {
    "SysvarRent111111111111111111111111111111111"
        .parse()
        .unwrap()
}

fn setup() -> (LiteSVM, Keypair) {
    let test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let deploy_dir = test_dir.join("target/deploy");
    build_program(
        test_dir.join("programs/spl").to_str().unwrap(),
        deploy_dir.to_str().unwrap(),
    );

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(program_id(), deploy_dir.join("spl_test.so"))
        .expect("load spl_test program");
    svm.add_program_from_file(token_2022_program_id(), token_2022_mainnet_so_path())
        .expect("load vendored mainnet Token-2022 program");

    let payer = keypair_for("spl-payer");
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
    (svm, payer)
}

fn data_with_mint_expectation(disc: u8, authority: &Pubkey, supply: u64, decimals: u8) -> Vec<u8> {
    let mut data = vec![disc];
    data.extend_from_slice(&authority.to_bytes());
    data.extend_from_slice(&supply.to_le_bytes());
    data.push(decimals);
    data
}

fn data_with_token_account_expectation(
    disc: u8,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Vec<u8> {
    let mut data = vec![disc];
    data.extend_from_slice(&mint.to_bytes());
    data.extend_from_slice(&owner.to_bytes());
    data.extend_from_slice(&amount.to_le_bytes());
    data
}

/// Build the send_instruction args for `init_mint` (discrim = 0).
fn do_init_mint(svm: &mut LiteSVM, payer: &Keypair, mint_kp: &Keypair, authority: &Pubkey) {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new(mint_kp.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![0], metas, payer, &[mint_kp])
        .expect("init_mint should succeed");
}

/// Build and dispatch `init_token_account` (discrim = 1).
fn do_init_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    token_kp: &Keypair,
    authority: &Pubkey,
) {
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(*mint, false),
        AccountMeta::new_readonly(*authority, false),
        AccountMeta::new(token_kp.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(svm, program_id(), vec![1], metas, payer, &[token_kp])
        .expect("init_token_account should succeed");
}

// ---- Init tests ------------------------------------------------------------

#[test]
fn init_mint_creates_mint_with_expected_state() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-authority");
    let mint = Keypair::new();

    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    let account = svm.get_account(&mint.pubkey()).expect("mint exists");
    assert_eq!(account.owner, token_program_id());
    assert_eq!(account.data.len(), SplMint::LEN);

    let state = SplMint::unpack(&account.data).expect("unpack mint");
    assert_eq!(state.decimals, 6);
    assert_eq!(state.supply, 0);
    assert!(state.is_initialized);
    // spl-token uses solana_program::pubkey::Pubkey; compare by bytes.
    let mint_authority_bytes = match state.mint_authority {
        spl_token::solana_program::program_option::COption::Some(pk) => pk.to_bytes(),
        spl_token::solana_program::program_option::COption::None => [0u8; 32],
    };
    assert_eq!(mint_authority_bytes, authority.pubkey().to_bytes());
}

#[test]
fn init_token_account_creates_account_with_expected_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-authority");
    let owner = keypair_for("token-owner");
    let mint = Keypair::new();
    let token = Keypair::new();

    do_init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());
    do_init_token_account(&mut svm, &payer, &mint.pubkey(), &token, &owner.pubkey());

    let account = svm.get_account(&token.pubkey()).expect("token exists");
    assert_eq!(account.owner, token_program_id());
    assert_eq!(account.data.len(), SplTokenAccount::LEN);

    let state = SplTokenAccount::unpack(&account.data).expect("unpack token");
    assert_eq!(state.mint.to_bytes(), mint.pubkey().to_bytes());
    assert_eq!(state.owner.to_bytes(), owner.pubkey().to_bytes());
    assert_eq!(state.amount, 0);
}

// ---- CPI operations --------------------------------------------------------

/// Shared fixture: mint with authority = `authority` + token account owned
/// by `owner` + 100 tokens minted to it.
fn mint_and_fund(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_authority: &Keypair,
    owner: &Pubkey,
    mint_amount: u64,
) -> (Pubkey, Pubkey) {
    let mint = Keypair::new();
    let token = Keypair::new();
    do_init_mint(svm, payer, &mint, &mint_authority.pubkey());
    do_init_token_account(svm, payer, &mint.pubkey(), &token, owner);

    // do_mint_to (discrim = 2)
    let mut data = vec![2];
    data.extend_from_slice(&mint_amount.to_le_bytes());
    let metas = vec![
        AccountMeta::new(mint.pubkey(), false),
        AccountMeta::new(token.pubkey(), false),
        AccountMeta::new_readonly(mint_authority.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(svm, program_id(), data, metas, payer, &[mint_authority])
        .expect("mint_to should succeed");
    (mint.pubkey(), token.pubkey())
}

#[test]
fn mint_to_increases_supply_and_balance() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 500);

    let mint_state = SplMint::unpack(&svm.get_account(&mint).unwrap().data).expect("unpack mint");
    assert_eq!(mint_state.supply, 500);

    let token_state =
        SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).expect("unpack token");
    assert_eq!(token_state.amount, 500);
}

#[test]
fn transfer_moves_tokens_between_accounts() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let recipient = keypair_for("recipient");

    let (mint, from) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 1000);
    let to = Keypair::new();
    do_init_token_account(&mut svm, &payer, &mint, &to, &recipient.pubkey());

    // do_transfer (discrim = 3)
    let mut data = vec![3];
    data.extend_from_slice(&250u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new(to.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("transfer should succeed");

    let from_state = SplTokenAccount::unpack(&svm.get_account(&from).unwrap().data).unwrap();
    let to_state = SplTokenAccount::unpack(&svm.get_account(&to.pubkey()).unwrap().data).unwrap();
    assert_eq!(from_state.amount, 750);
    assert_eq!(to_state.amount, 250);
}

#[test]
fn transfer_checked_validates_decimals() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let recipient = keypair_for("recipient");

    let (mint, from) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 1000);
    let to = Keypair::new();
    do_init_token_account(&mut svm, &payer, &mint, &to, &recipient.pubkey());

    // do_transfer_checked (discrim = 4), decimals = 6 (matches init_mint)
    let mut data = vec![4];
    data.extend_from_slice(&100u64.to_le_bytes());
    data.push(6);
    let metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(to.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("transfer_checked should succeed");

    let to_state = SplTokenAccount::unpack(&svm.get_account(&to.pubkey()).unwrap().data).unwrap();
    assert_eq!(to_state.amount, 100);
}

#[test]
fn burn_reduces_supply_and_balance() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 1000);

    // do_burn (discrim = 5)
    let mut data = vec![5];
    data.extend_from_slice(&400u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("burn should succeed");

    let mint_state = SplMint::unpack(&svm.get_account(&mint).unwrap().data).unwrap();
    let token_state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(mint_state.supply, 600);
    assert_eq!(token_state.amount, 600);
}

#[test]
fn approve_then_revoke_updates_delegate() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let delegate = keypair_for("delegate");
    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 1000);

    // do_approve (discrim = 6)
    let mut data = vec![6];
    data.extend_from_slice(&300u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(delegate.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("approve should succeed");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    let delegate_bytes = match state.delegate {
        spl_token::solana_program::program_option::COption::Some(pk) => pk.to_bytes(),
        spl_token::solana_program::program_option::COption::None => [0u8; 32],
    };
    assert_eq!(delegate_bytes, delegate.pubkey().to_bytes());
    assert_eq!(state.delegated_amount, 300);

    // do_revoke (discrim = 7)
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![7], metas, &payer, &[&owner])
        .expect("revoke should succeed");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert!(matches!(
        state.delegate,
        spl_token::solana_program::program_option::COption::None,
    ));
    assert_eq!(state.delegated_amount, 0);
}

#[test]
fn close_account_returns_lamports_to_destination() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    let dest = keypair_for("dest");
    let dest_before = svm
        .get_account(&dest.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);
    let token_lamports = svm.get_account(&token).unwrap().lamports;

    // do_close_account (discrim = 8)
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(dest.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![8], metas, &payer, &[&owner])
        .expect("close_account should succeed");

    assert!(svm.get_account(&token).is_none() || svm.get_account(&token).unwrap().lamports == 0);
    let dest_after = svm.get_account(&dest.pubkey()).unwrap().lamports;
    assert_eq!(dest_after, dest_before + token_lamports);
}

#[test]
fn checked_mint_burn_and_approve_helpers_update_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("checked-mint-auth");
    let owner = keypair_for("checked-owner");
    let delegate = keypair_for("checked-delegate");
    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 1000);

    let mut data = vec![77];
    data.extend_from_slice(&125u64.to_le_bytes());
    data.push(6);
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(mint_authority.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        data,
        metas,
        &payer,
        &[&mint_authority],
    )
    .expect("mint_to_checked should mint when decimals match");

    let mut data = vec![79];
    data.extend_from_slice(&400u64.to_le_bytes());
    data.push(6);
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(delegate.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("approve_checked should set delegate allowance when decimals match");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    let delegate_bytes = match state.delegate {
        COption::Some(pk) => pk.to_bytes(),
        COption::None => [0u8; 32],
    };
    assert_eq!(state.amount, 1125);
    assert_eq!(state.delegated_amount, 400);
    assert_eq!(delegate_bytes, delegate.pubkey().to_bytes());

    let mut data = vec![78];
    data.extend_from_slice(&225u64.to_le_bytes());
    data.push(6);
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("burn_checked should burn when decimals match");

    let mint_state = SplMint::unpack(&svm.get_account(&mint).unwrap().data).unwrap();
    let token_state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(mint_state.supply, 900);
    assert_eq!(token_state.amount, 900);
}

#[test]
fn freeze_and_thaw_helpers_update_account_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("freeze-mint-auth");
    let freeze_authority = keypair_for("freeze-authority");
    let owner = keypair_for("freeze-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint_with_freeze(&mint_authority.pubkey(), &freeze_authority.pubkey(), 6, 0)
            .to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        pack_base_token_account(&mint, &owner.pubkey(), 0).to_vec(),
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(freeze_authority.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![80],
        metas,
        &payer,
        &[&freeze_authority],
    )
    .expect("freeze_account should mark token account frozen");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(state.state, AccountState::Frozen);

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(freeze_authority.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![81],
        metas,
        &payer,
        &[&freeze_authority],
    )
    .expect("thaw_account should restore initialized state");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(state.state, AccountState::Initialized);
}

#[test]
fn sync_native_helper_updates_wrapped_sol_amount() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("native-owner");
    let token = Pubkey::new_unique();
    let reserve = svm.minimum_balance_for_rent_exemption(SplTokenAccount::LEN);
    let synced_amount = 12_345u64;

    svm.set_account(
        token,
        Account {
            lamports: reserve + synced_amount,
            data: pack_native_token_account(&owner.pubkey(), 0, reserve).to_vec(),
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed wrapped SOL account");

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![82], metas, &payer, &[])
        .expect("sync_native should derive amount from lamports minus rent reserve");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(state.is_native, COption::Some(reserve));
    assert_eq!(state.amount, synced_amount);
}

#[test]
fn initialize_mint_helpers_write_expected_state() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("raw-initialize-mint-authority");
    let mint_with_rent = Pubkey::new_unique();
    let mint_without_rent = Pubkey::new_unique();

    seed_uninitialized_token_owned_account(&mut svm, mint_with_rent, SplMint::LEN);
    seed_uninitialized_token_owned_account(&mut svm, mint_without_rent, SplMint::LEN);

    let mut data = vec![83, 5];
    data.extend_from_slice(&authority.pubkey().to_bytes());
    let metas = vec![
        AccountMeta::new(mint_with_rent, false),
        AccountMeta::new_readonly(rent_sysvar_id(), false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("initialize_mint should initialize a token-owned mint");

    let mut data = vec![84, 7];
    data.extend_from_slice(&authority.pubkey().to_bytes());
    let metas = vec![
        AccountMeta::new(mint_without_rent, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("initialize_mint2 should initialize a token-owned mint without rent sysvar");

    let mint_a = SplMint::unpack(&svm.get_account(&mint_with_rent).unwrap().data).unwrap();
    let mint_b = SplMint::unpack(&svm.get_account(&mint_without_rent).unwrap().data).unwrap();
    assert_eq!(mint_a.decimals, 5);
    assert_eq!(mint_b.decimals, 7);
    assert_eq!(mint_a.supply, 0);
    assert_eq!(mint_b.supply, 0);
    assert_eq!(
        mint_a.mint_authority,
        COption::Some(to_spl(&authority.pubkey()))
    );
    assert_eq!(
        mint_b.mint_authority,
        COption::Some(to_spl(&authority.pubkey()))
    );
}

#[test]
fn initialize_account_helpers_write_expected_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("raw-init-account-mint-auth");
    let owner = keypair_for("raw-init-account-owner");
    let mint = Keypair::new();
    let account_with_rent = Pubkey::new_unique();
    let account_without_rent = Pubkey::new_unique();

    do_init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());
    seed_uninitialized_token_owned_account(&mut svm, account_with_rent, SplTokenAccount::LEN);
    seed_uninitialized_token_owned_account(&mut svm, account_without_rent, SplTokenAccount::LEN);

    let metas = vec![
        AccountMeta::new(account_with_rent, false),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new_readonly(rent_sysvar_id(), false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![85], metas, &payer, &[])
        .expect("initialize_account should initialize a token-owned account");

    let metas = vec![
        AccountMeta::new(account_without_rent, false),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![86], metas, &payer, &[])
        .expect("initialize_account3 should initialize a token-owned account without rent sysvar");

    for token in [account_with_rent, account_without_rent] {
        let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
        assert_eq!(state.mint.to_bytes(), mint.pubkey().to_bytes());
        assert_eq!(state.owner.to_bytes(), owner.pubkey().to_bytes());
        assert_eq!(state.amount, 0);
        assert_eq!(state.state, AccountState::Initialized);
    }
}

#[test]
fn set_close_authority_helper_updates_token_account() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("set-authority-mint-auth");
    let owner = keypair_for("set-authority-owner");
    let close_authority = keypair_for("set-authority-close");
    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    let mut data = vec![87];
    data.extend_from_slice(&close_authority.pubkey().to_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner])
        .expect("set_authority should set token close authority");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(
        state.close_authority,
        COption::Some(to_spl(&close_authority.pubkey()))
    );
}

#[test]
fn token_cpi_ext_mint_to_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let recipient = keypair_for("token-cpi-ext-pda-mint-recipient");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint(&authority, 6, 0).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        pack_base_token_account(&mint, &recipient.pubkey(), 0).to_vec(),
    );

    let mut data = vec![93];
    data.extend_from_slice(&50u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::mint_to should sign for PDA authorities");

    let mint_state = SplMint::unpack(&svm.get_account(&mint).unwrap().data).unwrap();
    let token_state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(mint_state.supply, 50);
    assert_eq!(token_state.amount, 50);
}

#[test]
fn token_cpi_ext_transfer_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let recipient = keypair_for("token-cpi-ext-pda-transfer-recipient");
    let mint = Pubkey::new_unique();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        from,
        token_program_id(),
        pack_base_token_account(&mint, &authority, 80).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        to,
        token_program_id(),
        pack_base_token_account(&mint, &recipient.pubkey(), 0).to_vec(),
    );

    let mut data = vec![94];
    data.extend_from_slice(&25u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new(to, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::transfer should sign for PDA authorities");

    let from_state = SplTokenAccount::unpack(&svm.get_account(&from).unwrap().data).unwrap();
    let to_state = SplTokenAccount::unpack(&svm.get_account(&to).unwrap().data).unwrap();
    assert_eq!(from_state.amount, 55);
    assert_eq!(to_state.amount, 25);
}

#[test]
fn token_cpi_ext_set_authority_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();
    let new_close_authority = keypair_for("token-cpi-ext-pda-set-authority-new-close");
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        pack_base_token_account_with_close_authority(&mint, &authority, 0, &authority).to_vec(),
    );

    let mut data = vec![95];
    data.extend_from_slice(&new_close_authority.pubkey().to_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::set_authority should sign for PDA authorities");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(
        state.close_authority,
        COption::Some(to_spl(&new_close_authority.pubkey()))
    );
}

#[test]
fn token_cpi_ext_transfer_checked_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let recipient = keypair_for("token-cpi-ext-pda-transfer-checked-recipient");
    let mint = Pubkey::new_unique();
    let from = Pubkey::new_unique();
    let to = Pubkey::new_unique();
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint(&authority, 6, 0).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        from,
        token_program_id(),
        pack_base_token_account(&mint, &authority, 80).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        to,
        token_program_id(),
        pack_base_token_account(&mint, &recipient.pubkey(), 0).to_vec(),
    );

    let mut data = vec![96];
    data.extend_from_slice(&25u64.to_le_bytes());
    data.push(6);
    let metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(to, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::transfer_checked should sign for PDA authorities");

    let from_state = SplTokenAccount::unpack(&svm.get_account(&from).unwrap().data).unwrap();
    let to_state = SplTokenAccount::unpack(&svm.get_account(&to).unwrap().data).unwrap();
    assert_eq!(from_state.amount, 55);
    assert_eq!(to_state.amount, 25);
}

#[test]
fn token_cpi_ext_burn_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint(&authority, 6, 80).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        pack_base_token_account(&mint, &authority, 80).to_vec(),
    );

    let mut data = vec![97];
    data.extend_from_slice(&30u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::burn should sign for PDA authorities");

    let mint_state = SplMint::unpack(&svm.get_account(&mint).unwrap().data).unwrap();
    let token_state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(mint_state.supply, 50);
    assert_eq!(token_state.amount, 50);
}

#[test]
fn token_cpi_ext_approve_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let source = Pubkey::new_unique();
    let delegate = keypair_for("token-cpi-ext-pda-approve-delegate");
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint(&authority, 6, 0).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        source,
        token_program_id(),
        pack_base_token_account(&mint, &authority, 80).to_vec(),
    );

    let mut data = vec![98];
    data.extend_from_slice(&40u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(source, false),
        AccountMeta::new_readonly(delegate.pubkey(), false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::approve should sign for PDA authorities");

    let state = SplTokenAccount::unpack(&svm.get_account(&source).unwrap().data).unwrap();
    assert_eq!(state.delegate, COption::Some(to_spl(&delegate.pubkey())));
    assert_eq!(state.delegated_amount, 40);
}

#[test]
fn token_cpi_ext_thaw_supports_pda_authority() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("token-cpi-ext-pda-thaw-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();
    let (authority, _bump) = token_cpi_ext_authority_pda();

    seed_token_owned_account(&mut svm, authority, program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint_with_freeze(&owner.pubkey(), &authority, 6, 0).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        pack_frozen_token_account(&mint, &owner.pubkey(), 10).to_vec(),
    );

    let data = vec![99];
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(authority, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TokenCpiExt::thaw_account should sign for PDA authorities");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(state.state, AccountState::Initialized);
}

// ---- Accessor methods ------------------------------------------------------

#[test]
fn read_mint_asserts_expected_state() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-authority");
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    let metas = vec![AccountMeta::new_readonly(mint.pubkey(), false)];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_mint_expectation(9, &authority.pubkey(), 0, 6),
        metas,
        &payer,
        &[],
    )
    .expect("read_mint should succeed");
}

#[test]
fn read_mint_rejects_uninitialized_token_owned_mint() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();

    seed_token_owned_account(&mut svm, mint, token_program_id(), vec![0; SplMint::LEN]);

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_uninitialized_account_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(9, &Pubkey::new_unique(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "uninitialized Token-owned mint should not load as Account<Mint>",
    );
}

#[test]
fn read_mint_rejects_invalid_coption_tag() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-invalid-coption-auth");
    let mint = Pubkey::new_unique();
    let mut data = pack_base_mint(&authority.pubkey(), 6, 0).to_vec();

    // Mint authority COption tag must be exactly [0,0,0,0] or [1,0,0,0].
    data[1] = 2;
    seed_token_owned_account(&mut svm, mint, token_program_id(), data);

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(9, &authority.pubkey(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "mint with malformed COption tag should not load as Account<Mint>",
    );
}

#[test]
fn read_mint_rejects_invalid_initialized_state() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-invalid-state-auth");
    let mint = Pubkey::new_unique();
    let mut data = pack_base_mint(&authority.pubkey(), 6, 0).to_vec();

    const MINT_INITIALIZED_OFFSET: usize = 36 + 8 + 1;
    data[MINT_INITIALIZED_OFFSET] = 2;
    seed_token_owned_account(&mut svm, mint, token_program_id(), data);

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(9, &authority.pubkey(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "mint with invalid initialized byte should not load as Account<Mint>",
    );
}

#[test]
fn read_token_account_asserts_expected_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 100);

    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_token_account_expectation(10, &mint, &owner.pubkey(), 100),
        metas,
        &payer,
        &[],
    )
    .expect("read_token_account should succeed");
}

#[test]
fn read_token_account_rejects_uninitialized_token_owned_account() {
    let (mut svm, payer) = setup();
    let token = Pubkey::new_unique();

    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        vec![0; SplTokenAccount::LEN],
    );

    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_uninitialized_account_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_token_account_expectation(
                10,
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                0,
            ),
            metas,
            &payer,
            &[],
        ),
        "uninitialized Token-owned token account should not load as Account<TokenAccount>",
    );
}

#[test]
fn read_token_account_rejects_invalid_coption_tag() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let owner = keypair_for("token-invalid-coption-owner");
    let token = Pubkey::new_unique();
    let mut data = pack_base_token_account(&mint, &owner.pubkey(), 0).to_vec();

    const TOKEN_ACCOUNT_DELEGATE_TAG_OFFSET: usize = 32 + 32 + 8;
    data[TOKEN_ACCOUNT_DELEGATE_TAG_OFFSET + 1] = 2;
    seed_token_owned_account(&mut svm, token, token_program_id(), data);

    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_token_account_expectation(10, &mint, &owner.pubkey(), 0),
            metas,
            &payer,
            &[],
        ),
        "token account with malformed COption tag should not load as Account<TokenAccount>",
    );
}

#[test]
fn read_token_account_rejects_invalid_state() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let owner = keypair_for("token-invalid-state-owner");
    let token = Pubkey::new_unique();
    let mut data = pack_base_token_account(&mint, &owner.pubkey(), 0).to_vec();

    const TOKEN_ACCOUNT_STATE_OFFSET: usize = 32 + 32 + 8 + 4 + 32;
    data[TOKEN_ACCOUNT_STATE_OFFSET] = 3;
    seed_token_owned_account(&mut svm, token, token_program_id(), data);

    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_token_account_expectation(10, &mint, &owner.pubkey(), 0),
            metas,
            &payer,
            &[],
        ),
        "token account with invalid state byte should not load as Account<TokenAccount>",
    );
}

#[test]
fn mint_accessors_decode_non_default_state() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("rich-mint-authority");
    let freeze_authority = keypair_for("rich-mint-freeze-authority");
    let mint = Pubkey::new_unique();

    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint_with_freeze(&authority.pubkey(), &freeze_authority.pubkey(), 4, 987)
            .to_vec(),
    );

    let mut data = vec![88];
    data.extend_from_slice(&authority.pubkey().to_bytes());
    data.extend_from_slice(&freeze_authority.pubkey().to_bytes());
    data.extend_from_slice(&987u64.to_le_bytes());
    data.push(4);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("mint accessors should decode authority, freeze authority, supply, and decimals");
}

#[test]
fn token_account_accessors_decode_delegate_native_close_and_frozen_state() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let owner = keypair_for("rich-token-owner");
    let delegate = keypair_for("rich-token-delegate");
    let close_authority = keypair_for("rich-token-close-authority");
    let token = Pubkey::new_unique();
    let native_reserve = 2_039_280u64;

    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        pack_rich_token_account(
            &mint,
            &owner.pubkey(),
            55_000,
            &delegate.pubkey(),
            12_345,
            native_reserve,
            &close_authority.pubkey(),
        )
        .to_vec(),
    );

    let mut data = vec![89];
    data.extend_from_slice(&mint.to_bytes());
    data.extend_from_slice(&owner.pubkey().to_bytes());
    data.extend_from_slice(&55_000u64.to_le_bytes());
    data.extend_from_slice(&delegate.pubkey().to_bytes());
    data.extend_from_slice(&12_345u64.to_le_bytes());
    data.extend_from_slice(&close_authority.pubkey().to_bytes());
    data.extend_from_slice(&native_reserve.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "token account accessors should decode delegate, native reserve, close authority, and frozen state",
    );
}

// ---- Namespaced constraints ------------------------------------------------

#[test]
fn mint_decimals_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-auth");
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    // check_mint_decimals (discrim = 11) — init sets decimals = 6, matches.
    let metas = vec![AccountMeta::new(mint.pubkey(), false)];
    send_instruction(&mut svm, program_id(), vec![11], metas, &payer, &[])
        .expect("decimals match should pass");
}

#[test]
fn mint_authority_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-auth");
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    // check_mint_authority (discrim = 12): expected = `authority`, mint = mint.
    let metas = vec![
        AccountMeta::new_readonly(authority.pubkey(), false),
        AccountMeta::new(mint.pubkey(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![12], metas, &payer, &[])
        .expect("authority match should pass");
}

#[test]
fn mint_authority_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-auth");
    let wrong = keypair_for("wrong-authority");
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    let metas = vec![
        AccountMeta::new_readonly(wrong.pubkey(), false),
        AccountMeta::new(mint.pubkey(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![12], metas, &payer, &[]);
    assert!(result.is_err(), "mismatched authority should reject");
}

#[test]
fn token_mint_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    // check_token_mint (discrim = 13): pass the actual mint.
    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![13], metas, &payer, &[])
        .expect("token::mint match should pass");
}

#[test]
fn token_authority_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    // check_token_authority (discrim = 14): expected = owner.
    let metas = vec![
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![14], metas, &payer, &[])
        .expect("token::authority match should pass");
}

#[test]
fn strict_mint_freeze_authority_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("strict-freeze-mint-authority");
    let freeze_authority = keypair_for("strict-freeze-authority");
    let mint = Pubkey::new_unique();
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        pack_base_mint_with_freeze(&authority.pubkey(), &freeze_authority.pubkey(), 6, 0).to_vec(),
    );

    let metas = vec![
        AccountMeta::new_readonly(freeze_authority.pubkey(), false),
        AccountMeta::new(mint, false),
    ];
    send_instruction(&mut svm, program_id(), vec![90], metas, &payer, &[])
        .expect("strict mint::freeze_authority should accept the configured authority");
}

#[test]
fn strict_mint_freeze_authority_constraint_rejects_unset_and_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("strict-freeze-reject-mint-authority");
    let freeze_authority = keypair_for("strict-freeze-reject-authority");
    let wrong = keypair_for("strict-freeze-reject-wrong");
    let unset_mint = Pubkey::new_unique();
    let frozen_mint = Pubkey::new_unique();
    seed_token_owned_account(
        &mut svm,
        unset_mint,
        token_program_id(),
        pack_base_mint(&authority.pubkey(), 6, 0).to_vec(),
    );
    seed_token_owned_account(
        &mut svm,
        frozen_mint,
        token_program_id(),
        pack_base_mint_with_freeze(&authority.pubkey(), &freeze_authority.pubkey(), 6, 0).to_vec(),
    );

    let unset_metas = vec![
        AccountMeta::new_readonly(freeze_authority.pubkey(), false),
        AccountMeta::new(unset_mint, false),
    ];
    assert!(
        send_instruction(&mut svm, program_id(), vec![90], unset_metas, &payer, &[]).is_err(),
        "strict mint::freeze_authority should reject an unset freeze authority",
    );

    let mismatch_metas = vec![
        AccountMeta::new_readonly(wrong.pubkey(), false),
        AccountMeta::new(frozen_mint, false),
    ];
    assert!(
        send_instruction(
            &mut svm,
            program_id(),
            vec![90],
            mismatch_metas,
            &payer,
            &[]
        )
        .is_err(),
        "strict mint::freeze_authority should reject a mismatched authority",
    );
}

#[test]
fn strict_mint_token_program_constraint_checks_owner() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("strict-mint-token-program-authority");
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    let ok_metas = vec![
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(mint.pubkey(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![91], ok_metas, &payer, &[])
        .expect("strict mint::token_program should accept the owning token program");

    let bad_metas = vec![
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(mint.pubkey(), false),
    ];
    assert!(
        send_instruction(&mut svm, program_id(), vec![91], bad_metas, &payer, &[]).is_err(),
        "strict mint::token_program should reject a different owner program",
    );
}

#[test]
fn strict_token_account_token_program_constraint_checks_owner() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("strict-token-program-mint-authority");
    let owner = keypair_for("strict-token-program-owner");
    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    let ok_metas = vec![
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![92], ok_metas, &payer, &[])
        .expect("strict token::token_program should accept the owning token program");

    let bad_metas = vec![
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(token, false),
    ];
    assert!(
        send_instruction(&mut svm, program_id(), vec![92], bad_metas, &payer, &[]).is_err(),
        "strict token::token_program should reject a different owner program",
    );
}

// ---- CPI negative tests ---------------------------------------------------

#[test]
fn transfer_checked_rejects_wrong_decimals() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let recipient = keypair_for("recipient");

    let (mint, from) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 1000);
    let to = Keypair::new();
    do_init_token_account(&mut svm, &payer, &mint, &to, &recipient.pubkey());

    // Pass decimals=9 when mint has decimals=6
    let mut data = vec![4];
    data.extend_from_slice(&100u64.to_le_bytes());
    data.push(9); // wrong
    let metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(to.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner]);
    assert!(
        result.is_err(),
        "wrong decimals should be rejected by SPL token program"
    );
}

#[test]
fn transfer_rejects_insufficient_balance() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let recipient = keypair_for("recipient");

    let (mint, from) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 100);
    let to = Keypair::new();
    do_init_token_account(&mut svm, &payer, &mint, &to, &recipient.pubkey());

    // Try to transfer 200 when only 100 available
    let mut data = vec![3];
    data.extend_from_slice(&200u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(from, false),
        AccountMeta::new(to.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner]);
    assert!(result.is_err(), "overdraft should be rejected");
}

#[test]
fn mint_to_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let impostor = keypair_for("impostor");
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();
    let owner = keypair_for("owner");

    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    let mut data = vec![2];
    data.extend_from_slice(&500u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(impostor.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&impostor]);
    assert!(result.is_err(), "wrong mint authority should be rejected");

    // Verify nothing was minted
    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(
        state.amount, 0,
        "balance should be unchanged after failed mint"
    );
}

#[test]
fn burn_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let impostor = keypair_for("impostor");
    svm.airdrop(&impostor.pubkey(), 1_000_000_000).unwrap();

    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 500);

    let mut data = vec![5];
    data.extend_from_slice(&100u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(impostor.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&impostor]);
    assert!(result.is_err(), "wrong burn authority should be rejected");

    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(
        state.amount, 500,
        "balance should be unchanged after failed burn"
    );
}

#[test]
fn burn_rejects_wrong_token_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 500);
    let wrong_token_program = Pubkey::new_unique();

    let mut data = vec![44];
    data.extend_from_slice(&100u64.to_le_bytes());
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_program, false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&owner]);
    assert!(result.is_err(), "wrong token program should be rejected");

    let mint_state = SplMint::unpack(&svm.get_account(&mint).unwrap().data).unwrap();
    let state = SplTokenAccount::unpack(&svm.get_account(&token).unwrap().data).unwrap();
    assert_eq!(
        mint_state.supply, 500,
        "supply should be unchanged after failed burn"
    );
    assert_eq!(
        state.amount, 500,
        "balance should be unchanged after failed burn"
    );
}

#[test]
fn close_account_rejects_non_zero_balance() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 500);

    let dest = keypair_for("dest");
    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(dest.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![8], metas, &payer, &[&owner]);
    assert!(
        result.is_err(),
        "closing account with non-zero balance should be rejected"
    );
}

// ---- Constraint negative tests --------------------------------------------

#[test]
fn mint_decimals_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("mint-auth");

    // Create a normal mint with decimals=6 via our program
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    // Mutate the on-chain data to change decimals from 6 to 9.
    // Mint layout: [authority_flag:4][authority:32][supply:8][decimals:1][is_init:1][...]
    // decimals is at byte offset 44.
    let mut account = svm.get_account(&mint.pubkey()).expect("mint exists");
    assert_eq!(account.data[44], 6, "sanity: original decimals should be 6");
    account.data[44] = 9;
    svm.set_account(mint.pubkey(), account).unwrap();

    // check_mint_decimals (discrim=11) expects decimals=6
    let metas = vec![AccountMeta::new(mint.pubkey(), false)];
    let result = send_instruction(&mut svm, program_id(), vec![11], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "mint with decimals=9 should fail decimals=6 constraint"
    );
}

#[test]
fn token_mint_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");

    // Create two different mints
    let (mint_a, _token_a) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);
    let (_mint_b, token_b) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    // check_token_mint (discrim=13) with mint_a but token_b (which belongs to mint_b)
    let metas = vec![
        AccountMeta::new_readonly(mint_a, false),
        AccountMeta::new(token_b, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![13], metas, &payer, &[]);
    assert!(result.is_err(), "token::mint mismatch should be rejected");
}

#[test]
fn token_authority_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let owner = keypair_for("owner");
    let wrong = keypair_for("wrong-auth");

    let (_mint, token) = mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 0);

    // check_token_authority (discrim=14) with wrong expected authority
    let metas = vec![
        AccountMeta::new_readonly(wrong.pubkey(), false),
        AccountMeta::new(token, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![14], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "token::authority mismatch should be rejected"
    );
}

// ---- ATA derivation --------------------------------------------------------

#[test]
fn check_ata_accepts_canonical_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mint-auth");
    let wallet = keypair_for("ata-wallet");

    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    // Derive the canonical ATA and create it via our program.
    let ata = Pubkey::find_program_address(
        &[
            wallet.pubkey().as_ref(),
            token_program_id().as_ref(),
            mint.pubkey().as_ref(),
        ],
        &ata_program_id(),
    )
    .0;

    // Use the ATA program's Create instruction (idempotent create) so the
    // on-chain account matches the address the program derives.
    let create_ata_data = vec![0u8]; // Create discriminator
    let create_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(ata, false),
        AccountMeta::new_readonly(wallet.pubkey(), false),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        AccountMeta::new_readonly(token_program_id(), false),
    ];
    send_instruction(
        &mut svm,
        ata_program_id(),
        create_ata_data,
        create_metas,
        &payer,
        &[],
    )
    .expect("create ATA should succeed");

    // check_ata (discrim = 15) — passes if derivation matches `vault` addr.
    let metas = vec![
        AccountMeta::new_readonly(wallet.pubkey(), false),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(ata, false),
    ];
    send_instruction(&mut svm, program_id(), vec![15], metas, &payer, &[])
        .expect("canonical ATA should pass");
}

#[test]
fn check_ata_rejects_non_canonical_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("ata-rej-mint-auth");
    let wallet = keypair_for("ata-rej-wallet");
    let wrong_owner = keypair_for("ata-rej-owner");

    let mint = Keypair::new();
    let wrong_vault = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());
    do_init_token_account(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &wrong_vault,
        &wrong_owner.pubkey(),
    );

    let metas = vec![
        AccountMeta::new_readonly(wallet.pubkey(), false),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(wrong_vault.pubkey(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![15], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "non-canonical ATA address must be rejected"
    );
}

// ---- Token-2022 seeding helpers --------------------------------------------
//
// Build raw account data for Token-2022 extended mints and token accounts and
// drop it directly into the SVM via `set_account`. No CPI to Token-2022 is
// required because `InterfaceAccount` validation is ownership + length only;
// extension parsing is pure byte-level reading.

/// Convert our `solana_pubkey::Pubkey` into the `spl_token`-flavoured one so
/// `SplMint`/`SplTokenAccount` `pack` calls accept it.
fn to_spl(pk: &Pubkey) -> SplPubkey {
    SplPubkey::new_from_array(pk.to_bytes())
}

/// Build an 82-byte legacy `SplMint` state. Suitable for use as the base of
/// either a Token-2022 extended mint or a plain (legacy) mint account.
fn pack_base_mint(authority: &Pubkey, decimals: u8, supply: u64) -> [u8; SplMint::LEN] {
    let mint_state = SplMint {
        mint_authority: COption::Some(to_spl(authority)),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    let mut base = [0u8; SplMint::LEN];
    mint_state.pack_into_slice(&mut base);
    base
}

/// Like `pack_base_mint` but with `freeze_authority` set to `Some(freeze)`.
fn pack_base_mint_with_freeze(
    authority: &Pubkey,
    freeze: &Pubkey,
    decimals: u8,
    supply: u64,
) -> [u8; SplMint::LEN] {
    let mint_state = SplMint {
        mint_authority: COption::Some(to_spl(authority)),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: COption::Some(to_spl(freeze)),
    };
    let mut base = [0u8; SplMint::LEN];
    mint_state.pack_into_slice(&mut base);
    base
}

/// Build a 165-byte legacy `SplTokenAccount` state.
fn pack_base_token_account(
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> [u8; SplTokenAccount::LEN] {
    let state = SplTokenAccount {
        mint: to_spl(mint),
        owner: to_spl(owner),
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut base = [0u8; SplTokenAccount::LEN];
    state.pack_into_slice(&mut base);
    base
}

fn pack_frozen_token_account(
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> [u8; SplTokenAccount::LEN] {
    let state = SplTokenAccount {
        mint: to_spl(mint),
        owner: to_spl(owner),
        amount,
        delegate: COption::None,
        state: AccountState::Frozen,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut base = [0u8; SplTokenAccount::LEN];
    state.pack_into_slice(&mut base);
    base
}

fn pack_base_token_account_with_close_authority(
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
    close_authority: &Pubkey,
) -> [u8; SplTokenAccount::LEN] {
    let state = SplTokenAccount {
        mint: to_spl(mint),
        owner: to_spl(owner),
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::Some(to_spl(close_authority)),
    };
    let mut base = [0u8; SplTokenAccount::LEN];
    state.pack_into_slice(&mut base);
    base
}

fn pack_native_token_account(
    owner: &Pubkey,
    amount: u64,
    rent_exempt_reserve: u64,
) -> [u8; SplTokenAccount::LEN] {
    let state = SplTokenAccount {
        mint: to_spl(&native_mint_id()),
        owner: to_spl(owner),
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::Some(rent_exempt_reserve),
        delegated_amount: 0,
        close_authority: COption::None,
    };
    let mut base = [0u8; SplTokenAccount::LEN];
    state.pack_into_slice(&mut base);
    base
}

fn pack_rich_token_account(
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
    delegate: &Pubkey,
    delegated_amount: u64,
    native_reserve: u64,
    close_authority: &Pubkey,
) -> [u8; SplTokenAccount::LEN] {
    let state = SplTokenAccount {
        mint: to_spl(mint),
        owner: to_spl(owner),
        amount,
        delegate: COption::Some(to_spl(delegate)),
        state: AccountState::Frozen,
        is_native: COption::Some(native_reserve),
        delegated_amount,
        close_authority: COption::Some(to_spl(close_authority)),
    };
    let mut base = [0u8; SplTokenAccount::LEN];
    state.pack_into_slice(&mut base);
    base
}

/// Append a single TLV entry to `buf`: `u16_le type | u16_le length | value`.
fn push_tlv(buf: &mut Vec<u8>, ext_type: u16, value: &[u8]) {
    buf.extend_from_slice(&ext_type.to_le_bytes());
    buf.extend_from_slice(&(value.len() as u16).to_le_bytes());
    buf.extend_from_slice(value);
}

/// Build data for a Token-2022 extended mint: 82-byte base + zero pad to 165 +
/// `AccountType::Mint = 1` at byte 165 + caller-provided TLV region.
fn build_mint_data(authority: &Pubkey, decimals: u8, supply: u64, tlv: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(166 + tlv.len());
    data.extend_from_slice(&pack_base_mint(authority, decimals, supply));
    // pad to 165
    data.resize(165, 0);
    data.push(1); // AccountType::Mint
    data.extend_from_slice(tlv);
    data
}

/// Like `build_mint_data` but with `freeze_authority` set to `Some(freeze)`.
fn build_mint_data_with_freeze(
    authority: &Pubkey,
    freeze: &Pubkey,
    decimals: u8,
    supply: u64,
    tlv: &[u8],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(166 + tlv.len());
    data.extend_from_slice(&pack_base_mint_with_freeze(
        authority, freeze, decimals, supply,
    ));
    data.resize(165, 0);
    data.push(1); // AccountType::Mint
    data.extend_from_slice(tlv);
    data
}

/// Build data for a Token-2022 extended token account: 165-byte base +
/// `AccountType::Account = 2` at byte 165 + caller-provided TLV region.
fn build_token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64, tlv: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(166 + tlv.len());
    data.extend_from_slice(&pack_base_token_account(mint, owner, amount));
    data.push(2); // AccountType::Account
    data.extend_from_slice(tlv);
    data
}

/// Seed a Token-2022-owned account at `address` with the given raw bytes.
fn seed_token_2022_account(svm: &mut LiteSVM, address: Pubkey, data: Vec<u8>) {
    seed_token_owned_account(svm, address, token_2022_program_id(), data);
}

/// Seed a Token or Token-2022-owned account at `address` with the given bytes.
fn seed_token_owned_account(svm: &mut LiteSVM, address: Pubkey, owner: Pubkey, data: Vec<u8>) {
    svm.set_account(
        address,
        Account {
            lamports: 10_000_000,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed token-owned account");
}

fn seed_uninitialized_token_owned_account(svm: &mut LiteSVM, address: Pubkey, data_len: usize) {
    let lamports = svm.minimum_balance_for_rent_exemption(data_len);
    svm.set_account(
        address,
        Account {
            lamports,
            data: vec![0; data_len],
            owner: token_program_id(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .expect("seed uninitialized token-owned account");
}

fn seed_basic_token_2022_mint(svm: &mut LiteSVM, authority_label: &str) -> Pubkey {
    let authority = keypair_for(authority_label);
    let mint = Pubkey::new_unique();
    seed_token_2022_account(svm, mint, build_mint_data(&authority.pubkey(), 6, 0, &[]));
    mint
}

fn seed_uninitialized_token_2022_mint_with_extensions(
    svm: &mut LiteSVM,
    address: Pubkey,
    extensions: &[ExtensionType],
) {
    let len = ExtensionType::try_calculate_account_len::<Token2022Mint>(extensions)
        .expect("calculate Token-2022 mint extension account length");
    seed_token_2022_account(svm, address, vec![0; len]);
}

fn seed_uninitialized_token_2022_account_with_extensions(
    svm: &mut LiteSVM,
    address: Pubkey,
    extensions: &[ExtensionType],
) {
    let len = ExtensionType::try_calculate_account_len::<Token2022Account>(extensions)
        .expect("calculate Token-2022 token extension account length");
    seed_token_2022_account(svm, address, vec![0; len]);
}

fn seed_initialized_memo_transfer_account(
    svm: &mut LiteSVM,
    address: Pubkey,
    mint: Pubkey,
    owner: Pubkey,
) {
    let len = ExtensionType::try_calculate_account_len::<Token2022Account>(&[
        ExtensionType::MemoTransfer,
    ])
    .expect("calculate Token-2022 memo-transfer account length");
    let mut data = vec![0; len];
    let account = Token2022Account {
        mint,
        owner,
        amount: 0,
        delegate: Token2022COption::None,
        state: Token2022AccountState::Initialized,
        is_native: Token2022COption::None,
        delegated_amount: 0,
        close_authority: Token2022COption::None,
    };
    <Token2022Account as Token2022Pack>::pack_into_slice(
        &account,
        &mut data[..<Token2022Account as Token2022Pack>::LEN],
    );
    set_account_type::<Token2022Account>(&mut data).expect("set Token-2022 token account type");
    {
        let mut state = StateWithExtensionsMut::<Token2022Account>::unpack(&mut data)
            .expect("unpack memo-transfer token account");
        state
            .init_extension::<MemoTransfer>(false)
            .expect("initialize memo-transfer extension slot");
    }
    seed_token_2022_account(svm, address, data);
}

fn assert_uninitialized_account_error<T, E: std::fmt::Display>(
    result: Result<T, E>,
    context: &str,
) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("UninitializedAccount") || error.contains("uninitialized"),
        "{context}: expected UninitializedAccount, got:\n{error}"
    );
}

fn assert_invalid_account_data_error<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("InvalidAccountData") || error.contains("invalid account data"),
        "{context}: expected InvalidAccountData, got:\n{error}"
    );
}

fn assert_invalid_argument_error<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("InvalidArgument") || error.contains("invalid program argument"),
        "{context}: expected InvalidArgument, got:\n{error}"
    );
}

fn assert_incorrect_token_2022_program_error<T, E: std::fmt::Display>(
    result: Result<T, E>,
    context: &str,
) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("IncorrectProgramId") || error.contains("incorrect program id"),
        "{context}: expected IncorrectProgramId, got:\n{error}"
    );
}

fn assert_illegal_owner_error<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) {
    let Err(error) = result else {
        panic!("{context}");
    };
    let error = error.to_string();
    assert!(
        error.contains("IllegalOwner") || error.contains("illegal owner"),
        "{context}: expected IllegalOwner, got:\n{error}"
    );
}

// ---- InterfaceAccount validation -------------------------------------------

#[test]
fn read_interface_mint_accepts_legacy_token_owned() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("interface-mint-auth");
    let mint = Keypair::new();
    do_init_mint(&mut svm, &payer, &mint, &authority.pubkey());

    let metas = vec![AccountMeta::new_readonly(mint.pubkey(), false)];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_mint_expectation(16, &authority.pubkey(), 0, 6),
        metas,
        &payer,
        &[],
    )
    .expect("legacy-owned mint should pass interface load");
}

#[test]
fn read_interface_mint_accepts_token_2022_owned() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("t22-mint-auth");
    let mint = Pubkey::new_unique();

    let data = build_mint_data(&authority.pubkey(), 9, 0, &[]);
    seed_token_2022_account(&mut svm, mint, data);

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_mint_expectation(16, &authority.pubkey(), 0, 9),
        metas,
        &payer,
        &[],
    )
    .expect("token-2022-owned mint should pass interface load");
}

#[test]
fn read_interface_mint_rejects_uninitialized_legacy_and_token_2022_owned() {
    let (mut svm, payer) = setup();
    let legacy_mint = Pubkey::new_unique();
    let token_2022_mint = Pubkey::new_unique();

    seed_token_owned_account(
        &mut svm,
        legacy_mint,
        token_program_id(),
        vec![0; SplMint::LEN],
    );
    seed_token_owned_account(
        &mut svm,
        token_2022_mint,
        token_2022_program_id(),
        vec![0; SplMint::LEN],
    );

    let metas = vec![AccountMeta::new_readonly(legacy_mint, false)];
    assert_uninitialized_account_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(16, &Pubkey::new_unique(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "uninitialized legacy mint should not load as InterfaceAccount<Mint>",
    );

    let metas = vec![AccountMeta::new_readonly(token_2022_mint, false)];
    assert_uninitialized_account_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(16, &Pubkey::new_unique(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "uninitialized Token-2022 mint should not load as InterfaceAccount<Mint>",
    );
}

#[test]
fn read_interface_mint_rejects_token_2022_account_type_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mint-type-auth");
    let mint = Pubkey::new_unique();

    let mut data = build_mint_data(&authority.pubkey(), 6, 0, &[]);
    data[165] = 2; // AccountType::Account
    seed_token_2022_account(&mut svm, mint, data);

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(16, &authority.pubkey(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "Token-2022 account type marker must match InterfaceAccount<Mint>",
    );
}

#[test]
fn read_interface_mint_rejects_token_account() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let owner = keypair_for("iface-mint-cosplay-owner");
    let token = Pubkey::new_unique();

    // A 165-byte token account is longer than a Mint (82 bytes), so a
    // length-only check would accept it here. Regression test for #4510.
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 42, &[]),
    );

    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(16, &owner.pubkey(), 42, 6),
            metas,
            &payer,
            &[],
        ),
        "a token account must not load as InterfaceAccount<Mint>",
    );
}

#[test]
fn read_interface_mint_rejects_nonzero_token_2022_mint_padding() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mint-pad-auth");
    let mint = Pubkey::new_unique();

    let mut data = build_mint_data(&authority.pubkey(), 6, 0, &[]);
    data[82] = 1; // Mint extension padding before the AccountType marker.
    seed_token_2022_account(&mut svm, mint, data);

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_mint_expectation(16, &authority.pubkey(), 0, 6),
            metas,
            &payer,
            &[],
        ),
        "Token-2022 mint padding must be zero before the account type marker",
    );
}

#[test]
fn read_interface_mint_rejects_foreign_owner() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("foreign-mint-auth");
    let mint = Pubkey::new_unique();

    // Same bytes as a Token-2022 mint, but owned by an unrelated program.
    let data = build_mint_data(&authority.pubkey(), 6, 0, &[]);
    let foreign_owner = Pubkey::new_unique();
    svm.set_account(
        mint,
        Account {
            lamports: 10_000_000,
            data,
            owner: foreign_owner,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(
        &mut svm,
        program_id(),
        data_with_mint_expectation(16, &authority.pubkey(), 0, 6),
        metas,
        &payer,
        &[],
    );
    assert!(
        result.is_err(),
        "foreign-owned account should not load as InterfaceAccount<Mint>",
    );
}

#[test]
fn read_interface_token_account_accepts_legacy_and_token_2022() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mix-mint-auth");
    let owner = keypair_for("mix-owner");

    // Legacy branch: reuse the classic mint_and_fund flow.
    let (_mint, legacy_token) =
        mint_and_fund(&mut svm, &payer, &mint_authority, &owner.pubkey(), 10);
    let metas = vec![AccountMeta::new_readonly(legacy_token, false)];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_token_account_expectation(17, &_mint, &owner.pubkey(), 10),
        metas,
        &payer,
        &[],
    )
    .expect("legacy-owned token account should pass interface load");

    // Token-2022 branch: seed a raw extended account.
    let t22_mint = Pubkey::new_unique();
    let t22_token = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        t22_mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    seed_token_2022_account(
        &mut svm,
        t22_token,
        build_token_account_data(&t22_mint, &owner.pubkey(), 42, &[]),
    );
    let metas = vec![AccountMeta::new_readonly(t22_token, false)];
    send_instruction(
        &mut svm,
        program_id(),
        data_with_token_account_expectation(17, &t22_mint, &owner.pubkey(), 42),
        metas,
        &payer,
        &[],
    )
    .expect("token-2022-owned token account should pass interface load");
}

#[test]
fn read_interface_token_account_rejects_uninitialized_legacy_and_token_2022_owned() {
    let (mut svm, payer) = setup();
    let legacy_token = Pubkey::new_unique();
    let token_2022_token = Pubkey::new_unique();

    seed_token_owned_account(
        &mut svm,
        legacy_token,
        token_program_id(),
        vec![0; SplTokenAccount::LEN],
    );
    seed_token_owned_account(
        &mut svm,
        token_2022_token,
        token_2022_program_id(),
        vec![0; SplTokenAccount::LEN],
    );

    let metas = vec![AccountMeta::new_readonly(legacy_token, false)];
    assert_uninitialized_account_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_token_account_expectation(
                17,
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                0,
            ),
            metas,
            &payer,
            &[],
        ),
        "uninitialized legacy token account should not load as InterfaceAccount<TokenAccount>",
    );

    let metas = vec![AccountMeta::new_readonly(token_2022_token, false)];
    assert_uninitialized_account_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_token_account_expectation(
                17,
                &Pubkey::new_unique(),
                &Pubkey::new_unique(),
                0,
            ),
            metas,
            &payer,
            &[],
        ),
        "uninitialized Token-2022 token account should not load as InterfaceAccount<TokenAccount>",
    );
}

#[test]
fn read_interface_token_account_rejects_token_2022_account_type_mismatch() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let owner = keypair_for("iface-token-type-owner");
    let token = Pubkey::new_unique();

    let mut data = build_token_account_data(&mint, &owner.pubkey(), 42, &[]);
    data[165] = 1; // AccountType::Mint
    seed_token_2022_account(&mut svm, token, data);

    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_account_data_error(
        send_instruction(
            &mut svm,
            program_id(),
            data_with_token_account_expectation(17, &mint, &owner.pubkey(), 42),
            metas,
            &payer,
            &[],
        ),
        "Token-2022 account type marker must match InterfaceAccount<TokenAccount>",
    );
}

#[test]
fn read_interface_token_account_rejects_foreign_owner() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("foreign-token-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    svm.set_account(
        token,
        Account {
            lamports: 10_000_000,
            data: build_token_account_data(&mint, &owner.pubkey(), 42, &[]),
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    let metas = vec![AccountMeta::new_readonly(token, false)];
    let result = send_instruction(
        &mut svm,
        program_id(),
        data_with_token_account_expectation(17, &mint, &owner.pubkey(), 42),
        metas,
        &payer,
        &[],
    );
    assert!(
        result.is_err(),
        "foreign-owned account should not load as InterfaceAccount<TokenAccount>",
    );
}

// ---- InterfaceAccount init path --------------------------------------------

#[test]
fn init_interface_mint_creates_legacy_mint() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-init-mint-auth");
    let mint = Keypair::new();

    // init_interface_mint (discrim = 18)
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(authority.pubkey(), false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![18], metas, &payer, &[&mint])
        .expect("init_interface_mint should succeed");

    let account = svm.get_account(&mint.pubkey()).expect("mint exists");
    assert_eq!(account.owner, token_program_id());
    assert_eq!(account.data.len(), SplMint::LEN);
    let state = SplMint::unpack(&account.data).expect("unpack mint");
    assert_eq!(state.decimals, 6);
    assert!(state.is_initialized);
}

#[test]
fn init_interface_mint_creates_token_2022_mint() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-init-t22-mint-auth");
    let mint = Keypair::new();

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(authority.pubkey(), false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![18], metas, &payer, &[&mint])
        .expect("init token-2022 interface mint should succeed");

    let account = svm.get_account(&mint.pubkey()).expect("mint exists");
    assert_eq!(account.owner, token_2022_program_id());
    let state = SplMint::unpack(&account.data).expect("unpack mint");
    assert_eq!(state.decimals, 6);
    assert!(state.is_initialized);
}

#[test]
fn init_interface_mint_rejects_wrong_token_program() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-init-bad-mint-auth");
    let mint = Keypair::new();
    let wrong_token_program = Pubkey::new_unique();

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(authority.pubkey(), false),
        AccountMeta::new_readonly(wrong_token_program, false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    assert!(send_instruction(&mut svm, program_id(), vec![18], metas, &payer, &[&mint]).is_err());
    assert!(svm.get_account(&mint.pubkey()).is_none());
}

#[test]
fn init_interface_token_account_creates_legacy_token_account() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-init-token-mint-auth");
    let owner = keypair_for("iface-init-token-owner");
    let mint = Keypair::new();
    let token = Keypair::new();

    // Seed the mint through the non-interface init path so we don't double-
    // cover codegen; `Account<Mint>` coexists with `InterfaceAccount<Mint>`
    // on-chain — the underlying bytes are identical.
    do_init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    // init_interface_token_account (discrim = 19)
    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(token.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(&mut svm, program_id(), vec![19], metas, &payer, &[&token])
        .expect("init_interface_token_account should succeed");

    let account = svm.get_account(&token.pubkey()).expect("token exists");
    assert_eq!(account.owner, token_program_id());
    assert_eq!(account.data.len(), SplTokenAccount::LEN);
    let state = SplTokenAccount::unpack(&account.data).expect("unpack token");
    assert_eq!(state.mint.to_bytes(), mint.pubkey().to_bytes());
    assert_eq!(state.owner.to_bytes(), owner.pubkey().to_bytes());
}

#[test]
fn init_interface_token_account_rejects_wrong_token_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-init-bad-token-mint-auth");
    let owner = keypair_for("iface-init-bad-token-owner");
    let mint = Keypair::new();
    let token = Keypair::new();
    let wrong_token_program = Pubkey::new_unique();

    do_init_mint(&mut svm, &payer, &mint, &mint_authority.pubkey());

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new_readonly(wrong_token_program, false),
        AccountMeta::new(token.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    assert!(send_instruction(&mut svm, program_id(), vec![19], metas, &payer, &[&token]).is_err());
    assert!(svm.get_account(&token.pubkey()).is_none());
}

#[test]
fn init_interface_token_account_creates_token_2022_token_account() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-init-t22-token-mint-auth");
    let owner = keypair_for("iface-init-t22-token-owner");
    let mint = Keypair::new();
    let token = Keypair::new();

    let mint_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint_authority.pubkey(), false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(mint.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![18],
        mint_metas,
        &payer,
        &[&mint],
    )
    .expect("init token-2022 interface mint should succeed");

    let token_metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(mint.pubkey(), false),
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(token.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![19],
        token_metas,
        &payer,
        &[&token],
    )
    .expect("init token-2022 interface token account should succeed");

    let account = svm.get_account(&token.pubkey()).expect("token exists");
    assert_eq!(account.owner, token_2022_program_id());
    let state = SplTokenAccount::unpack(&account.data).expect("unpack token");
    assert_eq!(state.mint.to_bytes(), mint.pubkey().to_bytes());
    assert_eq!(state.owner.to_bytes(), owner.pubkey().to_bytes());
}

// ---- Namespaced constraints on InterfaceAccount ----------------------------

/// Shared fixture: a Token-2022-owned mint + token account pair. No
/// extensions — just the base state with the AccountType byte set.
fn seed_t22_mint_and_token(
    svm: &mut LiteSVM,
    mint_authority: &Pubkey,
    owner: &Pubkey,
) -> (Pubkey, Pubkey) {
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();
    seed_token_2022_account(svm, mint, build_mint_data(mint_authority, 6, 0, &[]));
    seed_token_2022_account(svm, token, build_token_account_data(&mint, owner, 0, &[]));
    (mint, token)
}

#[test]
fn interface_token_mint_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-tm-auth");
    let owner = keypair_for("iface-tm-owner");
    let (mint, token) =
        seed_t22_mint_and_token(&mut svm, &mint_authority.pubkey(), &owner.pubkey());

    // check_interface_token_mint (discrim = 20)
    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![20], metas, &payer, &[])
        .expect("matching token::mint should pass");
}

#[test]
fn interface_token_mint_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-tm-rej-auth");
    let owner = keypair_for("iface-tm-rej-owner");
    let (_real_mint, token) =
        seed_t22_mint_and_token(&mut svm, &mint_authority.pubkey(), &owner.pubkey());

    // A different mint (wrong one).
    let other_mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        other_mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let metas = vec![
        AccountMeta::new_readonly(other_mint, false),
        AccountMeta::new(token, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![20], metas, &payer, &[]);
    assert!(result.is_err(), "mismatched token::mint should reject");
}

#[test]
fn interface_token_authority_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-ta-auth");
    let owner = keypair_for("iface-ta-owner");
    let (_mint, token) =
        seed_t22_mint_and_token(&mut svm, &mint_authority.pubkey(), &owner.pubkey());

    // check_interface_token_authority (discrim = 21)
    let metas = vec![
        AccountMeta::new_readonly(owner.pubkey(), false),
        AccountMeta::new(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![21], metas, &payer, &[])
        .expect("matching token::authority should pass");
}

#[test]
fn interface_token_authority_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-ta-rej-auth");
    let owner = keypair_for("iface-ta-rej-owner");
    let wrong = keypair_for("iface-ta-rej-wrong");
    let (_mint, token) =
        seed_t22_mint_and_token(&mut svm, &mint_authority.pubkey(), &owner.pubkey());

    let metas = vec![
        AccountMeta::new_readonly(wrong.pubkey(), false),
        AccountMeta::new(token, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![21], metas, &payer, &[]);
    assert!(result.is_err(), "mismatched token::authority should reject");
}

#[test]
fn interface_token_program_constraint_accepts_token_2022() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-tp-auth");
    let owner = keypair_for("iface-tp-owner");
    let (_mint, token) =
        seed_t22_mint_and_token(&mut svm, &mint_authority.pubkey(), &owner.pubkey());

    // check_interface_token_program (discrim = 22): expected = Token-2022.
    let metas = vec![
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![22], metas, &payer, &[])
        .expect("token-2022-owned account should match Token-2022 program id");
}

#[test]
fn interface_token_program_constraint_rejects_wrong_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("iface-tp-rej-auth");
    let owner = keypair_for("iface-tp-rej-owner");
    let (_mint, token) =
        seed_t22_mint_and_token(&mut svm, &mint_authority.pubkey(), &owner.pubkey());

    // expected = legacy Token, actual owner = Token-2022 → reject.
    let metas = vec![
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(token, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![22], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "legacy Token expected vs Token-2022 owner should reject",
    );
}

#[test]
fn interface_mint_authority_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-ma-auth");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );

    // check_interface_mint_authority (discrim = 23)
    let metas = vec![
        AccountMeta::new_readonly(authority.pubkey(), false),
        AccountMeta::new(mint, false),
    ];
    send_instruction(&mut svm, program_id(), vec![23], metas, &payer, &[])
        .expect("matching mint::authority should pass");
}

#[test]
fn interface_mint_authority_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-ma-rej-auth");
    let wrong = keypair_for("iface-ma-rej-wrong");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );

    let metas = vec![
        AccountMeta::new_readonly(wrong.pubkey(), false),
        AccountMeta::new(mint, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![23], metas, &payer, &[]);
    assert!(result.is_err(), "mismatched mint::authority should reject");
}

#[test]
fn interface_mint_freeze_authority_constraint_rejects_when_unset() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mf-auth");
    let expected = keypair_for("iface-mf-expected");
    let mint = Pubkey::new_unique();
    // Base state uses COption::None for freeze_authority.
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );

    // check_interface_mint_freeze_authority (discrim = 24)
    let metas = vec![
        AccountMeta::new_readonly(expected.pubkey(), false),
        AccountMeta::new(mint, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![24], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "freeze_authority unset should fail the constraint",
    );
}

#[test]
fn interface_mint_freeze_authority_constraint_accepts_matching() {
    // Exercises the `Some(addr) if addr == expected` arm of
    // `mint::FreezeAuthorityConstraint::check`.
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mf-ok-auth");
    let expected = keypair_for("iface-mf-ok-expected");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data_with_freeze(&authority.pubkey(), &expected.pubkey(), 6, 0, &[]),
    );

    let metas = vec![
        AccountMeta::new_readonly(expected.pubkey(), false),
        AccountMeta::new(mint, false),
    ];
    send_instruction(&mut svm, program_id(), vec![24], metas, &payer, &[])
        .expect("matching mint::freeze_authority should pass");
}

#[test]
fn interface_mint_freeze_authority_constraint_rejects_mismatch() {
    // Exercises the `Some(addr) if addr != expected` (fall-through) arm
    // of `mint::FreezeAuthorityConstraint::check` — distinct from the
    // unset/None case covered above.
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mf-rej-auth");
    let expected = keypair_for("iface-mf-rej-expected");
    let other = keypair_for("iface-mf-rej-other");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data_with_freeze(&authority.pubkey(), &other.pubkey(), 6, 0, &[]),
    );

    let metas = vec![
        AccountMeta::new_readonly(expected.pubkey(), false),
        AccountMeta::new(mint, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![24], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "mismatched mint::freeze_authority should reject",
    );
}

#[test]
fn interface_mint_decimals_constraint_accepts_matching() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-md-auth");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );

    // check_interface_mint_decimals (discrim = 25) — expects 6.
    let metas = vec![AccountMeta::new(mint, false)];
    send_instruction(&mut svm, program_id(), vec![25], metas, &payer, &[])
        .expect("matching mint::decimals should pass");
}

#[test]
fn interface_mint_decimals_constraint_rejects_mismatch() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-md-rej-auth");
    let mint = Pubkey::new_unique();
    // Decimals = 9, but constraint expects 6.
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 9, 0, &[]),
    );

    let metas = vec![AccountMeta::new(mint, false)];
    let result = send_instruction(&mut svm, program_id(), vec![25], metas, &payer, &[]);
    assert!(result.is_err(), "mismatched mint::decimals should reject");
}

#[test]
fn interface_mint_token_program_constraint_accepts_token_2022() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mtp-auth");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );

    // check_interface_mint_token_program (discrim = 26): expected = Token-2022.
    let metas = vec![
        AccountMeta::new_readonly(token_2022_program_id(), false),
        AccountMeta::new(mint, false),
    ];
    send_instruction(&mut svm, program_id(), vec![26], metas, &payer, &[])
        .expect("token-2022-owned mint should match Token-2022 program id");
}

#[test]
fn interface_mint_token_program_constraint_rejects_wrong_program() {
    let (mut svm, payer) = setup();
    let authority = keypair_for("iface-mtp-rej-auth");
    let mint = Pubkey::new_unique();
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );

    // expected = legacy Token, actual owner = Token-2022 → reject.
    let metas = vec![
        AccountMeta::new_readonly(token_program_id(), false),
        AccountMeta::new(mint, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![26], metas, &payer, &[]);
    assert!(
        result.is_err(),
        "legacy Token expected vs Token-2022-owned mint should reject",
    );
}

// ---- Token-2022 extension parsing ------------------------------------------

/// Helpers for building TLV values matching the `anchor-spl::extensions`
/// struct layouts exactly. All fields are alignment-1, so raw byte-level
/// construction is safe.

fn tlv_transfer_fee_config(
    authority: &Pubkey,
    withdraw_authority: &Pubkey,
    newer_bps: u16,
    newer_epoch: u64,
    newer_max: u64,
) -> Vec<u8> {
    tlv_transfer_fee_config_full(
        authority,
        withdraw_authority,
        0,
        (0, 0, 0),
        (newer_epoch, newer_max, newer_bps),
    )
}

fn tlv_transfer_fee_config_full(
    authority: &Pubkey,
    withdraw_authority: &Pubkey,
    withheld_amount: u64,
    older: (u64, u64, u16),
    newer: (u64, u64, u16),
) -> Vec<u8> {
    let mut value = Vec::with_capacity(108);
    value.extend_from_slice(authority.as_ref());
    value.extend_from_slice(withdraw_authority.as_ref());
    value.extend_from_slice(&withheld_amount.to_le_bytes());
    // older_transfer_fee
    value.extend_from_slice(&older.0.to_le_bytes());
    value.extend_from_slice(&older.1.to_le_bytes());
    value.extend_from_slice(&older.2.to_le_bytes());
    // newer_transfer_fee
    value.extend_from_slice(&newer.0.to_le_bytes());
    value.extend_from_slice(&newer.1.to_le_bytes());
    value.extend_from_slice(&newer.2.to_le_bytes());
    let mut out = Vec::new();
    push_tlv(&mut out, 1, &value); // TransferFeeConfig
    out
}

fn append_transfer_fee_config_expectations(
    data: &mut Vec<u8>,
    withheld_amount: u64,
    older: (u64, u64, u16),
    newer: (u64, u64, u16),
) {
    data.extend_from_slice(&withheld_amount.to_le_bytes());
    data.extend_from_slice(&older.0.to_le_bytes());
    data.extend_from_slice(&older.1.to_le_bytes());
    data.extend_from_slice(&older.2.to_le_bytes());
    data.extend_from_slice(&newer.0.to_le_bytes());
    data.extend_from_slice(&newer.1.to_le_bytes());
    data.extend_from_slice(&newer.2.to_le_bytes());
}

fn tlv_metadata_pointer(authority: &Pubkey, metadata: &Pubkey) -> Vec<u8> {
    let mut value = Vec::with_capacity(64);
    value.extend_from_slice(authority.as_ref());
    value.extend_from_slice(metadata.as_ref());
    let mut out = Vec::new();
    push_tlv(&mut out, 18, &value); // MetadataPointer
    out
}

fn tlv_transfer_hook(authority: &Pubkey, program: &Pubkey) -> Vec<u8> {
    let mut value = Vec::with_capacity(64);
    value.extend_from_slice(authority.as_ref());
    value.extend_from_slice(program.as_ref());
    let mut out = Vec::new();
    push_tlv(&mut out, 14, &value); // TransferHook
    out
}

fn tlv_mint_close_authority(authority: &Pubkey) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 3, authority.as_ref()); // MintCloseAuthority
    out
}

fn tlv_mint_close_authority_none() -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 3, &[0u8; 32]); // MintCloseAuthority with no authority
    out
}

fn tlv_permanent_delegate(delegate: &Pubkey) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 12, delegate.as_ref()); // PermanentDelegate
    out
}

fn tlv_permanent_delegate_none() -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 12, &[0u8; 32]); // PermanentDelegate with no delegate
    out
}

fn tlv_transfer_fee_amount(withheld: u64) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 2, &withheld.to_le_bytes()); // TransferFeeAmount
    out
}

fn tlv_transfer_fee_amount_overlong(withheld: u64) -> Vec<u8> {
    let mut value = Vec::with_capacity(9);
    value.extend_from_slice(&withheld.to_le_bytes());
    value.push(0);
    let mut out = Vec::new();
    push_tlv(&mut out, 2, &value); // TransferFeeAmount with invalid length
    out
}

fn tlv_transfer_hook_account(transferring: u8) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 15, &[transferring]); // TransferHookAccount
    out
}

fn tlv_default_account_state(state: u8) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 6, &[state]); // DefaultAccountState
    out
}

fn tlv_group_pointer(authority: &Pubkey, group: &Pubkey) -> Vec<u8> {
    let mut value = Vec::with_capacity(64);
    value.extend_from_slice(authority.as_ref());
    value.extend_from_slice(group.as_ref());
    let mut out = Vec::new();
    push_tlv(&mut out, 20, &value); // GroupPointer
    out
}

fn tlv_group_member_pointer(authority: &Pubkey, member: &Pubkey) -> Vec<u8> {
    let mut value = Vec::with_capacity(64);
    value.extend_from_slice(authority.as_ref());
    value.extend_from_slice(member.as_ref());
    let mut out = Vec::new();
    push_tlv(&mut out, 22, &value); // GroupMemberPointer
    out
}

fn tlv_cpi_guard(enabled: u8) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, 11, &[enabled]); // CpiGuard
    out
}

fn tlv_pausable_config(authority: &Pubkey, paused: u8) -> Vec<u8> {
    let mut value = Vec::with_capacity(33);
    value.extend_from_slice(authority.as_ref());
    value.push(paused);
    let mut out = Vec::new();
    push_tlv(&mut out, 26, &value); // PausableConfig
    out
}

fn tlv_marker(ext_type: u16) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(&mut out, ext_type, &[]);
    out
}

fn concat_tlvs(tlvs: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for tlv in tlvs {
        out.extend_from_slice(tlv);
    }
    out
}

#[test]
fn transfer_fee_config_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("tfc-mint-auth");
    let fee_authority = keypair_for("tfc-fee-auth");
    let withdraw_authority = keypair_for("tfc-withdraw-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_fee_config(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        250, // basis points (2.5%)
        4,
        1_000_000,
    );
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    // read_transfer_fee_config (discrim = 27), expected bps = 250 → pass.
    let mut data = vec![27];
    append_transfer_fee_config_expectations(&mut data, 0, (0, 0, 0), (4, 1_000_000, 250));
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TransferFeeConfig bps should match");

    // Wrong bps → reject.
    let mut data = vec![27];
    append_transfer_fee_config_expectations(&mut data, 0, (0, 0, 0), (4, 1_000_000, 999));
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong bps should reject");
}

#[test]
fn transfer_fee_config_extension_decodes_all_fee_fields() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("tfc-full-mint-auth");
    let fee_authority = keypair_for("tfc-full-fee-auth");
    let withdraw_authority = keypair_for("tfc-full-withdraw-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_fee_config_full(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        321,
        (7, 9_999, 125),
        (11, 88_888, 250),
    );
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![27];
    append_transfer_fee_config_expectations(&mut data, 321, (7, 9_999, 125), (11, 88_888, 250));
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TransferFeeConfig accessors should decode withheld, older fee, and newer fee");
}

#[test]
fn unchecked_mint_extension_rejects_legacy_token_owner() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("unchecked-tfc-legacy-mint-auth");
    let fee_authority = keypair_for("unchecked-tfc-legacy-fee-auth");
    let withdraw_authority = keypair_for("unchecked-tfc-legacy-withdraw-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_fee_config(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        250,
        4,
        1_000_000,
    );
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    // read_unchecked_transfer_fee_config (discrim = 45), expected bps = 250.
    // The TLV bytes are valid, but extension helpers must only accept Token-2022.
    let mut data = vec![45];
    data.extend_from_slice(&250u16.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "unchecked mint extension should reject legacy Token-owned accounts",
    );
}

#[test]
fn unchecked_mint_extension_rejects_token_account_type_marker() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("unchecked-tfc-type-mint-auth");
    let fee_authority = keypair_for("unchecked-tfc-type-fee-auth");
    let withdraw_authority = keypair_for("unchecked-tfc-type-withdraw-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_fee_config(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        250,
        4,
        1_000_000,
    );
    let mut account_data = build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv);
    account_data[165] = 2; // AccountType::Account
    seed_token_2022_account(&mut svm, mint, account_data);

    let mut data = vec![45];
    data.extend_from_slice(&250u16.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_invalid_account_data_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "unchecked mint extension should reject token-account type marker",
    );
}

#[test]
fn unchecked_mint_extension_rejects_account_extension_family() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("unchecked-mint-account-family-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_fee_amount(777);
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let data = vec![53]; // read_unchecked_mint_transfer_fee_amount
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_invalid_account_data_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "unchecked mint extension should reject account extension family",
    );
}

#[test]
fn metadata_pointer_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mp-mint-auth");
    let meta_authority = keypair_for("mp-meta-auth");
    let metadata = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    let tlv = tlv_metadata_pointer(&meta_authority.pubkey(), &metadata);
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    // read_metadata_pointer (discrim = 28)
    let mut data = vec![28];
    data.extend_from_slice(&meta_authority.pubkey().to_bytes());
    data.extend_from_slice(&metadata.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("MetadataPointer should parse and match");
}

#[test]
fn metadata_pointer_extension_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mp-rej-mint-auth");
    let meta_authority = keypair_for("mp-rej-meta-auth");
    let wrong_authority = keypair_for("mp-rej-wrong-auth");
    let metadata = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    let tlv = tlv_metadata_pointer(&meta_authority.pubkey(), &metadata);
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![28];
    data.extend_from_slice(&wrong_authority.pubkey().to_bytes());
    data.extend_from_slice(&metadata.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "wrong metadata pointer authority should reject"
    );
}

#[test]
fn metadata_pointer_extension_rejects_wrong_metadata_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mp-rej2-mint-auth");
    let meta_authority = keypair_for("mp-rej2-meta-auth");
    let metadata = Pubkey::new_unique();
    let wrong_metadata = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    let tlv = tlv_metadata_pointer(&meta_authority.pubkey(), &metadata);
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![28];
    data.extend_from_slice(&meta_authority.pubkey().to_bytes());
    data.extend_from_slice(&wrong_metadata.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "wrong metadata pointer target should reject"
    );
}

#[test]
fn transfer_hook_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("th-mint-auth");
    let hook_authority = keypair_for("th-hook-auth");
    let hook_program = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_hook(&hook_authority.pubkey(), &hook_program);
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    // read_transfer_hook (discrim = 29)
    let mut data = vec![29];
    data.extend_from_slice(&hook_program.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TransferHook program id should match");
}

#[test]
fn transfer_hook_extension_rejects_wrong_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("th-rej-mint-auth");
    let hook_authority = keypair_for("th-rej-hook-auth");
    let hook_program = Pubkey::new_unique();
    let wrong_program = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_hook(&hook_authority.pubkey(), &hook_program);
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![29];
    data.extend_from_slice(&wrong_program.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong transfer hook program should reject");
}

#[test]
fn mint_close_authority_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mca-mint-auth");
    let close_authority = keypair_for("mca-close-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_mint_close_authority(&close_authority.pubkey());
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    // read_mint_close_authority (discrim = 30) — exercises optional_address.
    let mut data = vec![30];
    data.extend_from_slice(&close_authority.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("MintCloseAuthority close authority should match");
}

#[test]
fn mint_close_authority_extension_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mca-rej-mint-auth");
    let close_authority = keypair_for("mca-rej-close-auth");
    let wrong_authority = keypair_for("mca-rej-wrong-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_mint_close_authority(&close_authority.pubkey());
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![30];
    data.extend_from_slice(&wrong_authority.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong close authority should reject");
}

#[test]
fn mint_close_authority_extension_rejects_when_missing() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mca-missing-mint-auth");
    let expected = keypair_for("mca-missing-expected");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let mut data = vec![30];
    data.extend_from_slice(&expected.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "missing close authority extension should reject"
    );
}

#[test]
fn mint_close_authority_extension_accepts_unset_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("mca-none-mint-auth");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_mint_close_authority_none(),
        ),
    );

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let mut data = vec![30];
    data.extend_from_slice(&[0u8; 32]);
    data.push(1);
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("MintCloseAuthority with an all-zero authority should decode as None");
}

#[test]
fn permanent_delegate_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pd-mint-auth");
    let delegate = keypair_for("pd-delegate");
    let mint = Pubkey::new_unique();

    let tlv = tlv_permanent_delegate(&delegate.pubkey());
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    // read_permanent_delegate (discrim = 31)
    let mut data = vec![31];
    data.extend_from_slice(&delegate.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("PermanentDelegate delegate should match");
}

#[test]
fn permanent_delegate_extension_rejects_wrong_delegate() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pd-rej-mint-auth");
    let delegate = keypair_for("pd-rej-delegate");
    let wrong_delegate = keypair_for("pd-rej-wrong-delegate");
    let mint = Pubkey::new_unique();

    let tlv = tlv_permanent_delegate(&delegate.pubkey());
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![31];
    data.extend_from_slice(&wrong_delegate.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong permanent delegate should reject");
}

#[test]
fn permanent_delegate_extension_rejects_when_missing() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pd-missing-mint-auth");
    let expected = keypair_for("pd-missing-expected");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let mut data = vec![31];
    data.extend_from_slice(&expected.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "missing permanent delegate extension should reject"
    );
}

#[test]
fn permanent_delegate_extension_accepts_unset_delegate() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pd-none-mint-auth");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_permanent_delegate_none(),
        ),
    );

    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let mut data = vec![31];
    data.extend_from_slice(&[0u8; 32]);
    data.push(1);
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("PermanentDelegate with an all-zero delegate should decode as None");
}

#[test]
fn transfer_fee_amount_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("tfa-mint-auth");
    let owner = keypair_for("tfa-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_fee_amount(777);
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    // read_transfer_fee_amount (discrim = 32)
    let mut data = vec![32];
    data.extend_from_slice(&777u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TransferFeeAmount withheld should match");
}

#[test]
fn token_interface_extension_trait_reads_mint_and_token_account_extensions() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("trait-ext-mint-auth");
    let fee_authority = keypair_for("trait-ext-fee-auth");
    let withdraw_authority = keypair_for("trait-ext-withdraw-auth");
    let owner = keypair_for("trait-ext-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    let mint_tlv = tlv_transfer_fee_config(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        250,
        4,
        1_000_000,
    );
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &mint_tlv),
    );

    let token_tlv = tlv_transfer_fee_amount(777);
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &token_tlv),
    );

    let mut data = vec![55]; // read_transfer_fee_config_via_trait
    data.extend_from_slice(&250u16.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("InterfaceAccount<Mint>::get_extension should read TransferFeeConfig");

    let mut data = vec![56]; // read_transfer_fee_amount_via_trait
    data.extend_from_slice(&777u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("InterfaceAccount<TokenAccount>::get_extension should read TransferFeeAmount");
}

#[test]
fn token_interface_mint_extension_trait_rejects_legacy_token_owner() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("trait-ext-legacy-mint-auth");
    let fee_authority = keypair_for("trait-ext-legacy-fee-auth");
    let withdraw_authority = keypair_for("trait-ext-legacy-withdraw-auth");
    let mint = Pubkey::new_unique();

    let tlv = tlv_transfer_fee_config(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        250,
        4,
        1_000_000,
    );
    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![55]; // read_transfer_fee_config_via_trait
    data.extend_from_slice(&250u16.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    assert_illegal_owner_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "InterfaceAccount<Mint>::get_extension should reject legacy Token owners",
    );
}

#[test]
fn token_interface_token_extension_trait_rejects_legacy_token_owner() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("trait-ext-legacy-token-mint-auth");
    let owner = keypair_for("trait-ext-legacy-token-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_owned_account(
        &mut svm,
        mint,
        token_program_id(),
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let tlv = tlv_transfer_fee_amount(777);
    seed_token_owned_account(
        &mut svm,
        token,
        token_program_id(),
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    let mut data = vec![56]; // read_transfer_fee_amount_via_trait
    data.extend_from_slice(&777u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_illegal_owner_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "InterfaceAccount<TokenAccount>::get_extension should reject legacy Token owners",
    );
}

#[test]
fn unchecked_token_extension_rejects_mint_account_type_marker() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("unchecked-tfa-type-mint-auth");
    let owner = keypair_for("unchecked-tfa-type-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_fee_amount(777);
    let mut account_data = build_token_account_data(&mint, &owner.pubkey(), 0, &tlv);
    account_data[165] = 1; // AccountType::Mint
    seed_token_2022_account(&mut svm, token, account_data);

    // read_unchecked_transfer_fee_amount (discrim = 46), expected withheld = 777.
    let mut data = vec![46];
    data.extend_from_slice(&777u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_account_data_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "unchecked token extension should reject mint type marker",
    );
}

#[test]
fn unchecked_token_extension_rejects_uninitialized_base_account() {
    let (mut svm, payer) = setup();
    let token = Pubkey::new_unique();
    let tlv = tlv_transfer_fee_amount(777);
    let mut account_data = vec![0; 166];
    account_data[165] = 2; // AccountType::Account
    account_data.extend_from_slice(&tlv);
    seed_token_2022_account(&mut svm, token, account_data);

    let mut data = vec![46];
    data.extend_from_slice(&777u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_uninitialized_account_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "unchecked token extension should reject uninitialized token base account",
    );
}

#[test]
fn unchecked_token_extension_rejects_overlong_fixed_extension_value() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("unchecked-tfa-overlong-mint-auth");
    let owner = keypair_for("unchecked-tfa-overlong-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_fee_amount_overlong(777);
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    let mut data = vec![46]; // read_unchecked_transfer_fee_amount
    data.extend_from_slice(&777u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_argument_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "unchecked token extension should reject overlong fixed-size TLV values",
    );
}

#[test]
fn unchecked_token_extension_rejects_mint_extension_family() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("unchecked-token-mint-family-mint-auth");
    let fee_authority = keypair_for("unchecked-token-mint-family-fee-auth");
    let withdraw_authority = keypair_for("unchecked-token-mint-family-withdraw-auth");
    let owner = keypair_for("unchecked-token-mint-family-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_fee_config(
        &fee_authority.pubkey(),
        &withdraw_authority.pubkey(),
        250,
        4,
        1_000_000,
    );
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    let data = vec![54]; // read_unchecked_token_account_transfer_fee_config
    let metas = vec![AccountMeta::new_readonly(token, false)];
    assert_invalid_account_data_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "unchecked token extension should reject mint extension family",
    );
}

#[test]
fn transfer_fee_amount_extension_rejects_wrong_withheld_amount() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("tfa-rej-mint-auth");
    let owner = keypair_for("tfa-rej-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_fee_amount(777);
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    let mut data = vec![32];
    data.extend_from_slice(&778u64.to_le_bytes());
    let metas = vec![AccountMeta::new_readonly(token, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong withheld amount should reject");
}

#[test]
fn transfer_hook_account_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("tha-mint-auth");
    let owner = keypair_for("tha-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_hook_account(1);
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    // read_transfer_hook_account (discrim = 33)
    let data = vec![33, 1];
    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("TransferHookAccount transferring should match");
}

#[test]
fn transfer_hook_account_extension_rejects_wrong_transferring_flag() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("tha-rej-mint-auth");
    let owner = keypair_for("tha-rej-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    let tlv = tlv_transfer_hook_account(1);
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv),
    );

    let data = vec![33, 0];
    let metas = vec![AccountMeta::new_readonly(token, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong transferring flag should reject");
}

#[test]
fn missing_extension_returns_invalid_account_data() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("noext-mint-auth");
    let mint = Pubkey::new_unique();

    // Mint has MintCloseAuthority only — lookup for TransferFeeConfig fails.
    let other = keypair_for("noext-close-auth");
    let tlv = tlv_mint_close_authority(&other.pubkey());
    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv),
    );

    let mut data = vec![27]; // read_transfer_fee_config
    append_transfer_fee_config_expectations(&mut data, 0, (0, 0, 0), (4, 1_000_000, 250));
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "missing extension should surface InvalidAccountData",
    );
}

#[test]
fn default_account_state_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("das-mint-auth");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_default_account_state(2),
        ),
    );

    let data = vec![34, 2];
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("DefaultAccountState should parse and match");

    let data = vec![34, 1];
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong default state should reject");
}

#[test]
fn group_pointer_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("gp-mint-auth");
    let group_authority = keypair_for("gp-authority");
    let group = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_pointer(&group_authority.pubkey(), &group),
        ),
    );

    let mut data = vec![35];
    data.extend_from_slice(&group_authority.pubkey().to_bytes());
    data.extend_from_slice(&group.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("GroupPointer should parse and match");
}

#[test]
fn group_pointer_extension_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("gp-rej-mint-auth");
    let group_authority = keypair_for("gp-rej-authority");
    let wrong_authority = keypair_for("gp-rej-wrong-authority");
    let group = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_pointer(&group_authority.pubkey(), &group),
        ),
    );

    let mut data = vec![35];
    data.extend_from_slice(&wrong_authority.pubkey().to_bytes());
    data.extend_from_slice(&group.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "wrong group pointer authority should reject"
    );
}

#[test]
fn group_pointer_extension_rejects_wrong_group_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("gp-rej2-mint-auth");
    let group_authority = keypair_for("gp-rej2-authority");
    let group = Pubkey::new_unique();
    let wrong_group = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_pointer(&group_authority.pubkey(), &group),
        ),
    );

    let mut data = vec![35];
    data.extend_from_slice(&group_authority.pubkey().to_bytes());
    data.extend_from_slice(&wrong_group.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong group pointer address should reject");
}

#[test]
fn group_member_pointer_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("gmp-mint-auth");
    let member_authority = keypair_for("gmp-authority");
    let member = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_member_pointer(&member_authority.pubkey(), &member),
        ),
    );

    let mut data = vec![36];
    data.extend_from_slice(&member_authority.pubkey().to_bytes());
    data.extend_from_slice(&member.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("GroupMemberPointer should parse and match");
}

#[test]
fn group_member_pointer_extension_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("gmp-rej-mint-auth");
    let member_authority = keypair_for("gmp-rej-authority");
    let wrong_authority = keypair_for("gmp-rej-wrong-authority");
    let member = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_member_pointer(&member_authority.pubkey(), &member),
        ),
    );

    let mut data = vec![36];
    data.extend_from_slice(&wrong_authority.pubkey().to_bytes());
    data.extend_from_slice(&member.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "wrong group member pointer authority should reject"
    );
}

#[test]
fn group_member_pointer_extension_rejects_wrong_member_address() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("gmp-rej2-mint-auth");
    let member_authority = keypair_for("gmp-rej2-authority");
    let member = Pubkey::new_unique();
    let wrong_member = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_member_pointer(&member_authority.pubkey(), &member),
        ),
    );

    let mut data = vec![36];
    data.extend_from_slice(&member_authority.pubkey().to_bytes());
    data.extend_from_slice(&wrong_member.to_bytes());
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(
        result.is_err(),
        "wrong group member pointer address should reject"
    );
}

#[test]
fn cpi_guard_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("cg-mint-auth");
    let owner = keypair_for("cg-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv_cpi_guard(1)),
    );

    let data = vec![37, 1];
    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("CpiGuard should parse and match");

    let data = vec![37, 0];
    let metas = vec![AccountMeta::new_readonly(token, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong CPI guard state should reject");
}

#[test]
fn cpi_guard_extension_accepts_disabled_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("cg-disabled-mint-auth");
    let owner = keypair_for("cg-disabled-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &tlv_cpi_guard(0)),
    );

    let data = vec![37, 0];
    let metas = vec![AccountMeta::new_readonly(token, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("CpiGuard with a zero lock byte should decode as disabled");
}

#[test]
fn pausable_config_extension_round_trips() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pause-mint-auth");
    let pause_authority = keypair_for("pause-authority");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_pausable_config(&pause_authority.pubkey(), 1),
        ),
    );

    let mut data = vec![38];
    data.extend_from_slice(&pause_authority.pubkey().to_bytes());
    data.push(1);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("PausableConfig should parse and match");
}

#[test]
fn pausable_config_extension_accepts_unpaused_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pause-unpaused-mint-auth");
    let pause_authority = keypair_for("pause-unpaused-authority");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_pausable_config(&pause_authority.pubkey(), 0),
        ),
    );

    let mut data = vec![38];
    data.extend_from_slice(&pause_authority.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("PausableConfig with a zero paused byte should decode as unpaused");
}

#[test]
fn pausable_config_extension_rejects_wrong_authority() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pause-rej-mint-auth");
    let pause_authority = keypair_for("pause-rej-authority");
    let wrong_authority = keypair_for("pause-rej-wrong-authority");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_pausable_config(&pause_authority.pubkey(), 1),
        ),
    );

    let mut data = vec![38];
    data.extend_from_slice(&wrong_authority.pubkey().to_bytes());
    data.push(1);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong pause authority should reject");
}

#[test]
fn pausable_config_extension_rejects_wrong_paused_state() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("pause-rej2-mint-auth");
    let pause_authority = keypair_for("pause-rej2-authority");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_pausable_config(&pause_authority.pubkey(), 1),
        ),
    );

    let mut data = vec![38];
    data.extend_from_slice(&pause_authority.pubkey().to_bytes());
    data.push(0);
    let metas = vec![AccountMeta::new_readonly(mint, false)];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[]);
    assert!(result.is_err(), "wrong paused state should reject");
}

#[test]
fn zero_sized_marker_extensions_round_trip() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("marker-mint-auth");
    let owner = keypair_for("marker-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &tlv_marker(9)),
    );
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(
            &mint,
            &owner.pubkey(),
            0,
            &concat_tlvs(&[tlv_marker(13), tlv_marker(27)]),
        ),
    );

    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token, false),
    ];
    send_instruction(&mut svm, program_id(), vec![39], metas, &payer, &[])
        .expect("zero-sized marker extensions should parse");
}

#[test]
fn zero_sized_marker_extensions_reject_when_marker_missing() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("marker-rej-mint-auth");
    let owner = keypair_for("marker-rej-owner");
    let mint = Pubkey::new_unique();
    let token = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );
    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(
            &mint,
            &owner.pubkey(),
            0,
            &concat_tlvs(&[tlv_marker(13), tlv_marker(27)]),
        ),
    );

    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token, false),
    ];
    let result = send_instruction(&mut svm, program_id(), vec![39], metas, &payer, &[]);
    assert!(result.is_err(), "missing mint marker should reject");
}

#[test]
fn group_pointer_update_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-gp-mint-auth");
    let authority = keypair_for("token-2022-gp-authority");
    let group = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_pointer(&authority.pubkey(), &group),
        ),
    );

    let mut data = vec![41];
    data.extend_from_slice(&group.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority]);
    assert!(
        result.is_err(),
        "group pointer update helper should reject non-Token-2022 program ids before CPI"
    );
}

#[test]
fn group_pointer_update_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-gp-ok-mint-auth");
    let authority = keypair_for("token-2022-gp-ok-authority");
    let group = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_pointer(&authority.pubkey(), &group),
        ),
    );

    let mut data = vec![41];
    data.extend_from_slice(&group.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority])
        .expect("group pointer update helper should invoke the canonical Token-2022 program");
}

#[test]
fn group_member_pointer_update_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-gmp-mint-auth");
    let authority = keypair_for("token-2022-gmp-authority");
    let member = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_member_pointer(&authority.pubkey(), &member),
        ),
    );

    let mut data = vec![42];
    data.extend_from_slice(&member.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    let result = send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority]);
    assert!(
        result.is_err(),
        "group member pointer update helper should reject non-Token-2022 program ids before CPI"
    );
}

#[test]
fn group_member_pointer_update_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-gmp-ok-mint-auth");
    let authority = keypair_for("token-2022-gmp-ok-authority");
    let member = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(
            &mint_authority.pubkey(),
            6,
            0,
            &tlv_group_member_pointer(&authority.pubkey(), &member),
        ),
    );

    let mut data = vec![42];
    data.extend_from_slice(&member.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[&authority]).expect(
        "group member pointer update helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn reallocate_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-realloc-mint-auth");
    let owner = keypair_for("token-2022-realloc-owner");
    let token = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &[]),
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        AccountMeta::new_readonly(mint_authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    let result = send_instruction(
        &mut svm,
        program_id(),
        vec![43],
        metas,
        &payer,
        &[&mint_authority],
    );
    assert!(
        result.is_err(),
        "reallocate helper should reject non-Token-2022 program ids before CPI"
    );
}

#[test]
fn reallocate_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("token-2022-realloc-ok-owner");
    let token = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &[]),
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![43], metas, &payer, &[&owner])
        .expect("reallocate helper should invoke the canonical Token-2022 program");
}

#[test]
fn token_2022_immutable_owner_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let token = Pubkey::new_unique();
    seed_uninitialized_token_2022_account_with_extensions(
        &mut svm,
        token,
        &[ExtensionType::ImmutableOwner],
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![68], metas, &payer, &[]).expect(
        "direct Token-2022 immutable owner helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn token_2022_mint_close_authority_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let close_authority = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::MintCloseAuthority],
    );

    let mut data = vec![69];
    data.extend_from_slice(&close_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "direct Token-2022 mint close authority helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn token_2022_non_transferable_mint_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::NonTransferable],
    );

    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![70], metas, &payer, &[]).expect(
        "direct Token-2022 non-transferable mint helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn token_2022_permanent_delegate_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let permanent_delegate = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::PermanentDelegate],
    );

    let mut data = vec![71];
    data.extend_from_slice(&permanent_delegate.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "direct Token-2022 permanent delegate helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn token_2022_get_account_data_size_helper_reads_return_data() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(
        &mut svm,
        "token-2022-token-2022-account-data-size-mint-auth",
    );

    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![72], metas, &payer, &[])
        .expect("direct Token-2022 get_account_data_size helper should decode callee return data");
}

#[test]
fn token_2022_amount_to_ui_amount_helper_reads_return_data() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-token-2022-amount-to-ui-mint-auth");

    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![73], metas, &payer, &[])
        .expect("direct Token-2022 amount_to_ui_amount helper should decode callee return data");
}

#[test]
fn token_2022_ui_amount_to_amount_helper_reads_return_data() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-token-2022-ui-to-amount-mint-auth");

    let metas = vec![
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![75], metas, &payer, &[])
        .expect("direct Token-2022 ui_amount_to_amount helper should decode callee return data");
}

#[test]
fn immutable_owner_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("token-2022-immutable-owner-reject-owner");
    let token = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &[]),
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), vec![57], metas, &payer, &[]),
        "immutable owner initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn immutable_owner_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let token = Pubkey::new_unique();
    seed_uninitialized_token_2022_account_with_extensions(
        &mut svm,
        token,
        &[ExtensionType::ImmutableOwner],
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![57], metas, &payer, &[])
        .expect("immutable owner initialize helper should invoke the canonical Token-2022 program");
}

#[test]
fn non_transferable_mint_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-non-transferable-reject-mint-auth");
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), vec![58], metas, &payer, &[]),
        "non-transferable mint initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn non_transferable_mint_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::NonTransferable],
    );

    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![58], metas, &payer, &[]).expect(
        "non-transferable mint initialize helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn mint_close_authority_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-close-authority-reject-mint-auth");
    let close_authority = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let mut data = vec![59];
    data.extend_from_slice(&close_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "mint close authority initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn mint_close_authority_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let close_authority = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::MintCloseAuthority],
    );

    let mut data = vec![59];
    data.extend_from_slice(&close_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "mint close authority initialize helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn permanent_delegate_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint_authority = keypair_for("token-2022-permanent-delegate-reject-mint-auth");
    let permanent_delegate = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        mint,
        build_mint_data(&mint_authority.pubkey(), 6, 0, &[]),
    );

    let mut data = vec![60];
    data.extend_from_slice(&permanent_delegate.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "permanent delegate initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn permanent_delegate_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let permanent_delegate = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::PermanentDelegate],
    );

    let mut data = vec![60];
    data.extend_from_slice(&permanent_delegate.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "permanent delegate initialize helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn default_account_state_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-default-state-reject-mint-auth");

    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), vec![61], metas, &payer, &[]),
        "default account state initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn default_account_state_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::DefaultAccountState],
    );

    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![61], metas, &payer, &[]).expect(
        "default account state initialize helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn memo_transfer_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("token-2022-memo-transfer-reject-owner");
    let token = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_token_2022_account(
        &mut svm,
        token,
        build_token_account_data(&mint, &owner.pubkey(), 0, &[]),
    );

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), vec![62], metas, &payer, &[&owner]),
        "memo transfer initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn memo_transfer_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let owner = keypair_for("token-2022-memo-transfer-ok-owner");
    let token = Pubkey::new_unique();
    let mint = Pubkey::new_unique();

    seed_initialized_memo_transfer_account(&mut svm, token, mint, owner.pubkey());

    let metas = vec![
        AccountMeta::new(token, false),
        AccountMeta::new_readonly(owner.pubkey(), true),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![62], metas, &payer, &[&owner])
        .expect("memo transfer initialize helper should invoke the canonical Token-2022 program");
}

#[test]
fn metadata_pointer_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-metadata-pointer-reject-mint-auth");
    let authority = Pubkey::new_unique();
    let metadata_address = Pubkey::new_unique();

    let mut data = vec![63];
    data.extend_from_slice(&authority.to_bytes());
    data.extend_from_slice(&metadata_address.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "metadata pointer initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn metadata_pointer_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let metadata_address = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::MetadataPointer],
    );

    let mut data = vec![63];
    data.extend_from_slice(&authority.to_bytes());
    data.extend_from_slice(&metadata_address.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "metadata pointer initialize helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn transfer_hook_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-transfer-hook-reject-mint-auth");
    let authority = Pubkey::new_unique();
    let hook_program = Pubkey::new_unique();

    let mut data = vec![64];
    data.extend_from_slice(&authority.to_bytes());
    data.extend_from_slice(&hook_program.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "transfer hook initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn transfer_hook_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let hook_program = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::TransferHook],
    );

    let mut data = vec![64];
    data.extend_from_slice(&authority.to_bytes());
    data.extend_from_slice(&hook_program.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("transfer hook initialize helper should invoke the canonical Token-2022 program");
}

#[test]
fn interest_bearing_mint_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-interest-bearing-reject-mint-auth");
    let rate_authority = Pubkey::new_unique();

    let mut data = vec![65];
    data.extend_from_slice(&rate_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "interest-bearing mint initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn interest_bearing_mint_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let rate_authority = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::InterestBearingConfig],
    );

    let mut data = vec![65];
    data.extend_from_slice(&rate_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[]).expect(
        "interest-bearing mint initialize helper should invoke the canonical Token-2022 program",
    );
}

#[test]
fn pausable_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-pausable-reject-mint-auth");
    let authority = Pubkey::new_unique();

    let mut data = vec![66];
    data.extend_from_slice(&authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "pausable initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn pausable_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(&mut svm, mint, &[ExtensionType::Pausable]);

    let mut data = vec![66];
    data.extend_from_slice(&authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("pausable initialize helper should invoke the canonical Token-2022 program");
}

#[test]
fn transfer_fee_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = seed_basic_token_2022_mint(&mut svm, "token-2022-transfer-fee-reject-mint-auth");
    let config_authority = Pubkey::new_unique();

    let mut data = vec![67];
    data.extend_from_slice(&config_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), data, metas, &payer, &[]),
        "transfer fee initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn transfer_fee_initialize_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let mint = Pubkey::new_unique();
    let config_authority = Pubkey::new_unique();
    seed_uninitialized_token_2022_mint_with_extensions(
        &mut svm,
        mint,
        &[ExtensionType::TransferFeeConfig],
    );

    let mut data = vec![67];
    data.extend_from_slice(&config_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(mint, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), data, metas, &payer, &[])
        .expect("transfer fee initialize helper should invoke the canonical Token-2022 program");
}

#[test]
fn create_native_mint_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    seed_token_owned_account(
        &mut svm,
        token_2022_native_mint_id(),
        solana_sdk_ids::system_program::ID,
        vec![],
    );

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(token_2022_native_mint_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(&mut svm, program_id(), vec![100], metas, &payer, &[]),
        "create_native_mint helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn create_native_mint_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    seed_token_owned_account(
        &mut svm,
        token_2022_native_mint_id(),
        solana_sdk_ids::system_program::ID,
        vec![],
    );

    let metas = vec![
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new(token_2022_native_mint_id(), false),
        AccountMeta::new_readonly(solana_sdk_ids::system_program::ID, false),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(&mut svm, program_id(), vec![100], metas, &payer, &[])
        .expect("create_native_mint helper should invoke the canonical Token-2022 program");
}

#[test]
fn withdraw_excess_lamports_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let source = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let authority = keypair_for("token-2022-withdraw-excess-reject-authority");

    seed_token_owned_account(&mut svm, source, token_2022_program_id(), vec![]);
    seed_token_owned_account(
        &mut svm,
        destination,
        solana_sdk_ids::system_program::ID,
        vec![],
    );

    let metas = vec![
        AccountMeta::new(source, false),
        AccountMeta::new(destination, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            vec![101],
            metas,
            &payer,
            &[&authority],
        ),
        "withdraw_excess_lamports helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn withdraw_excess_lamports_helper_invokes_token_2022_program() {
    let (mut svm, payer) = setup();
    let source = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let authority = keypair_for("token-2022-withdraw-excess-ok-authority");

    seed_token_2022_account(
        &mut svm,
        source,
        build_mint_data(&authority.pubkey(), 6, 0, &[]),
    );
    seed_token_owned_account(
        &mut svm,
        destination,
        solana_sdk_ids::system_program::ID,
        vec![],
    );

    let metas = vec![
        AccountMeta::new(source, false),
        AccountMeta::new(destination, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new_readonly(token_2022_program_id(), false),
    ];
    send_instruction(
        &mut svm,
        program_id(),
        vec![101],
        metas,
        &payer,
        &[&authority],
    )
    .expect("withdraw_excess_lamports helper should invoke the canonical Token-2022 program");
}

#[test]
fn token_metadata_remove_key_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let metadata = Pubkey::new_unique();
    let update_authority = keypair_for("token-2022-meta-remove-authority");

    seed_token_owned_account(&mut svm, metadata, token_2022_program_id(), vec![0; 8]);

    let metas = vec![
        AccountMeta::new(metadata, false),
        AccountMeta::new_readonly(update_authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            vec![47],
            metas,
            &payer,
            &[&update_authority],
        ),
        "token metadata remove_key helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn token_metadata_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let metadata = Pubkey::new_unique();
    let update_authority = Pubkey::new_unique();
    let mint_authority = keypair_for("token-2022-meta-init-mint-authority");
    let mint = Pubkey::new_unique();

    seed_token_owned_account(&mut svm, metadata, token_2022_program_id(), vec![0; 8]);
    seed_token_owned_account(
        &mut svm,
        update_authority,
        token_2022_program_id(),
        vec![0; 8],
    );
    seed_token_owned_account(&mut svm, mint, token_2022_program_id(), vec![0; 8]);

    let metas = vec![
        AccountMeta::new(metadata, false),
        AccountMeta::new_readonly(update_authority, false),
        AccountMeta::new_readonly(mint_authority.pubkey(), true),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            vec![48],
            metas,
            &payer,
            &[&mint_authority],
        ),
        "token metadata initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn token_metadata_update_authority_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let metadata = Pubkey::new_unique();
    let current_authority = keypair_for("token-2022-meta-update-current-authority");
    let new_authority = Pubkey::new_unique();

    seed_token_owned_account(&mut svm, metadata, token_2022_program_id(), vec![0; 8]);

    let mut data = vec![49];
    data.extend_from_slice(&new_authority.to_bytes());
    let metas = vec![
        AccountMeta::new(metadata, false),
        AccountMeta::new_readonly(current_authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            data,
            metas,
            &payer,
            &[&current_authority],
        ),
        "token metadata update_authority helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn token_metadata_update_field_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let metadata = Pubkey::new_unique();
    let update_authority = keypair_for("token-2022-meta-update-field-authority");

    seed_token_owned_account(&mut svm, metadata, token_2022_program_id(), vec![0; 8]);

    let metas = vec![
        AccountMeta::new(metadata, false),
        AccountMeta::new_readonly(update_authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            vec![50],
            metas,
            &payer,
            &[&update_authority],
        ),
        "token metadata update_field helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn token_group_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let group = Pubkey::new_unique();
    let mint_authority = keypair_for("token-2022-group-init-mint-authority");
    let mint = Pubkey::new_unique();

    seed_token_owned_account(&mut svm, group, token_2022_program_id(), vec![0; 8]);
    seed_token_owned_account(&mut svm, mint, token_2022_program_id(), vec![0; 8]);

    let metas = vec![
        AccountMeta::new(group, false),
        AccountMeta::new_readonly(mint, false),
        AccountMeta::new_readonly(mint_authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            vec![51],
            metas,
            &payer,
            &[&mint_authority],
        ),
        "token group initialize helper should reject non-Token-2022 program ids before CPI",
    );
}

#[test]
fn token_member_initialize_helper_rejects_non_token_2022_program() {
    let (mut svm, payer) = setup();
    let member = Pubkey::new_unique();
    let member_mint = Pubkey::new_unique();
    let member_mint_authority = keypair_for("token-2022-member-init-mint-authority");
    let group = Pubkey::new_unique();
    let group_update_authority = keypair_for("token-2022-member-init-group-authority");

    seed_token_owned_account(&mut svm, member, token_2022_program_id(), vec![0; 8]);
    seed_token_owned_account(&mut svm, member_mint, token_2022_program_id(), vec![0; 8]);
    seed_token_owned_account(&mut svm, group, token_2022_program_id(), vec![0; 8]);

    let metas = vec![
        AccountMeta::new(member, false),
        AccountMeta::new_readonly(member_mint, false),
        AccountMeta::new_readonly(member_mint_authority.pubkey(), true),
        AccountMeta::new(group, false),
        AccountMeta::new_readonly(group_update_authority.pubkey(), true),
        AccountMeta::new_readonly(wrong_token_2022_program_id(), false),
    ];
    assert_incorrect_token_2022_program_error(
        send_instruction(
            &mut svm,
            program_id(),
            vec![52],
            metas,
            &payer,
            &[&member_mint_authority, &group_update_authority],
        ),
        "token member initialize helper should reject non-Token-2022 program ids before CPI",
    );
}
