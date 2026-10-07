//! System program CPI helpers.
//!
//! Mirrors the v1 `anchor_lang::system_program` CPI utility surface while
//! using v2 [`CpiContext`] and CPI handle accounts.

extern crate alloc;

pub use crate::programs::System;
use {
    crate::{require, CpiContext, CpiHandle, CpiHandleMut, Id, ToCpiAccounts},
    alloc::{string::String, vec::Vec},
    pinocchio::address::MAX_SEED_LEN,
    solana_address::Address,
    solana_program_error::{ProgramError, ProgramResult},
};

pub const ID: Address = crate::address!("11111111111111111111111111111111");

const NONCE_ACCOUNT_LENGTH: u64 = 80;

#[inline]
fn check_system_program(program: &Address) -> ProgramResult {
    require!(
        crate::address_eq(program, &System::id()),
        ProgramError::IncorrectProgramId
    );
    Ok(())
}

#[inline]
fn encode_u32(value: u32, data: &mut Vec<u8>) {
    data.extend_from_slice(&value.to_le_bytes());
}

#[inline]
fn encode_u64(value: u64, data: &mut Vec<u8>) {
    data.extend_from_slice(&value.to_le_bytes());
}

#[inline]
fn encode_address(value: &Address, data: &mut Vec<u8>) {
    data.extend_from_slice(value.as_ref());
}

fn checked_seed(seed: &str) -> Result<&[u8], ProgramError> {
    let seed = seed.as_bytes();
    if seed.len() > MAX_SEED_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(seed)
}

#[inline]
fn invoke<'a, T: ToCpiAccounts<'a>>(ctx: &CpiContext<'a, T>, data: &[u8]) -> ProgramResult {
    check_system_program(ctx.program)?;
    ctx.invoke(data)
}

pub fn advance_nonce_account<'a>(ctx: CpiContext<'a, AdvanceNonceAccount<'a>>) -> ProgramResult {
    invoke(&ctx, &4u32.to_le_bytes())
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct AdvanceNonceAccount<'a> {
    pub nonce: CpiHandleMut<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    #[signer]
    pub authorized: CpiHandle<'a>,
}

pub fn allocate<'a>(ctx: CpiContext<'a, Allocate<'a>>, space: u64) -> ProgramResult {
    let mut data = Vec::with_capacity(12);
    encode_u32(8, &mut data);
    encode_u64(space, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct Allocate<'a> {
    #[signer]
    pub account_to_allocate: CpiHandleMut<'a>,
}

pub fn allocate_with_seed<'a>(
    ctx: CpiContext<'a, AllocateWithSeed<'a>>,
    seed: &str,
    space: u64,
    owner: &Address,
) -> ProgramResult {
    let seed = checked_seed(seed)?;
    let mut data = Vec::with_capacity(84 + seed.len());
    encode_u32(9, &mut data);
    encode_address(ctx.accounts.base.address(), &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_u64(space, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct AllocateWithSeed<'a> {
    pub account_to_allocate: CpiHandleMut<'a>,
    #[signer]
    pub base: CpiHandle<'a>,
}

pub fn assign<'a>(ctx: CpiContext<'a, Assign<'a>>, owner: &Address) -> ProgramResult {
    let mut data = Vec::with_capacity(36);
    encode_u32(1, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct Assign<'a> {
    #[signer]
    pub account_to_assign: CpiHandleMut<'a>,
}

pub fn assign_with_seed<'a>(
    ctx: CpiContext<'a, AssignWithSeed<'a>>,
    seed: &str,
    owner: &Address,
) -> ProgramResult {
    let seed = checked_seed(seed)?;
    let mut data = Vec::with_capacity(76 + seed.len());
    encode_u32(10, &mut data);
    encode_address(ctx.accounts.base.address(), &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct AssignWithSeed<'a> {
    pub account_to_assign: CpiHandleMut<'a>,
    #[signer]
    pub base: CpiHandle<'a>,
}

pub fn authorize_nonce_account<'a>(
    ctx: CpiContext<'a, AuthorizeNonceAccount<'a>>,
    new_authority: &Address,
) -> ProgramResult {
    let mut data = Vec::with_capacity(36);
    encode_u32(7, &mut data);
    encode_address(new_authority, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct AuthorizeNonceAccount<'a> {
    pub nonce: CpiHandleMut<'a>,
    #[signer]
    pub authorized: CpiHandle<'a>,
}

pub fn create_account<'a>(
    ctx: CpiContext<'a, CreateAccount<'a>>,
    lamports: u64,
    space: u64,
    owner: &Address,
) -> ProgramResult {
    let mut data = Vec::with_capacity(52);
    encode_u32(0, &mut data);
    encode_u64(lamports, &mut data);
    encode_u64(space, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct CreateAccount<'a> {
    #[signer]
    pub from: CpiHandleMut<'a>,
    #[signer]
    pub to: CpiHandleMut<'a>,
}

pub fn create_account_with_seed<'a>(
    ctx: CpiContext<'a, CreateAccountWithSeed<'a>>,
    seed: &str,
    lamports: u64,
    space: u64,
    owner: &Address,
) -> ProgramResult {
    let seed = checked_seed(seed)?;
    let mut data = Vec::with_capacity(92 + seed.len());
    encode_u32(3, &mut data);
    encode_address(ctx.accounts.base.address(), &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_u64(lamports, &mut data);
    encode_u64(space, &mut data);
    encode_address(owner, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct CreateAccountWithSeed<'a> {
    #[signer]
    pub from: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub base: CpiHandle<'a>,
}

pub fn create_nonce_account<'a>(
    ctx: CpiContext<'a, CreateNonceAccount<'a>>,
    lamports: u64,
    authority: &Address,
) -> ProgramResult {
    let owner = System::id();
    let create_accounts = CreateAccount {
        from: ctx.accounts.from,
        to: ctx.accounts.nonce,
    };
    create_account(
        CpiContext::new_with_signer(ctx.program, create_accounts, ctx.signer_seeds),
        lamports,
        NONCE_ACCOUNT_LENGTH,
        &owner,
    )?;

    let initialize_accounts = InitializeNonceAccount {
        nonce: ctx.accounts.nonce,
        recent_blockhashes: ctx.accounts.recent_blockhashes,
        rent: ctx.accounts.rent,
    };
    initialize_nonce_account(
        CpiContext::new_with_signer(ctx.program, initialize_accounts, ctx.signer_seeds),
        authority,
    )
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct CreateNonceAccount<'a> {
    #[signer]
    pub from: CpiHandleMut<'a>,
    #[signer]
    pub nonce: CpiHandleMut<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

pub fn create_nonce_account_with_seed<'a>(
    ctx: CpiContext<'a, CreateNonceAccountWithSeed<'a>>,
    lamports: u64,
    seed: &str,
    authority: &Address,
) -> ProgramResult {
    let owner = System::id();
    let create_accounts = CreateAccountWithSeed {
        from: ctx.accounts.from,
        to: ctx.accounts.nonce,
        base: ctx.accounts.base,
    };
    create_account_with_seed(
        CpiContext::new_with_signer(ctx.program, create_accounts, ctx.signer_seeds),
        seed,
        lamports,
        NONCE_ACCOUNT_LENGTH,
        &owner,
    )?;

    let initialize_accounts = InitializeNonceAccount {
        nonce: ctx.accounts.nonce,
        recent_blockhashes: ctx.accounts.recent_blockhashes,
        rent: ctx.accounts.rent,
    };
    initialize_nonce_account(
        CpiContext::new_with_signer(ctx.program, initialize_accounts, ctx.signer_seeds),
        authority,
    )
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct CreateNonceAccountWithSeed<'a> {
    #[signer]
    pub from: CpiHandleMut<'a>,
    pub nonce: CpiHandleMut<'a>,
    #[signer]
    pub base: CpiHandle<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

pub fn transfer<'a>(ctx: CpiContext<'a, Transfer<'a>>, lamports: u64) -> ProgramResult {
    let mut data = Vec::with_capacity(12);
    encode_u32(2, &mut data);
    encode_u64(lamports, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct Transfer<'a> {
    #[signer]
    pub from: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
}

pub fn transfer_with_seed<'a>(
    ctx: CpiContext<'a, TransferWithSeed<'a>>,
    from_seed: String,
    from_owner: &Address,
    lamports: u64,
) -> ProgramResult {
    let seed = checked_seed(&from_seed)?;
    let mut data = Vec::with_capacity(52 + seed.len());
    encode_u32(11, &mut data);
    encode_u64(lamports, &mut data);
    encode_u64(seed.len() as u64, &mut data);
    data.extend_from_slice(seed);
    encode_address(from_owner, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct TransferWithSeed<'a> {
    pub from: CpiHandleMut<'a>,
    #[signer]
    pub base: CpiHandle<'a>,
    pub to: CpiHandleMut<'a>,
}

pub fn withdraw_nonce_account<'a>(
    ctx: CpiContext<'a, WithdrawNonceAccount<'a>>,
    lamports: u64,
) -> ProgramResult {
    let mut data = Vec::with_capacity(12);
    encode_u32(5, &mut data);
    encode_u64(lamports, &mut data);
    invoke(&ctx, &data)
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
pub struct WithdrawNonceAccount<'a> {
    pub nonce: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    pub recent_blockhashes: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
    #[signer]
    pub authorized: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
#[accounts_program_id(ID)]
struct InitializeNonceAccount<'a> {
    nonce: CpiHandleMut<'a>,
    recent_blockhashes: CpiHandle<'a>,
    rent: CpiHandle<'a>,
}

fn initialize_nonce_account<'a>(
    ctx: CpiContext<'a, InitializeNonceAccount<'a>>,
    authority: &Address,
) -> ProgramResult {
    let mut data = Vec::with_capacity(36);
    encode_u32(6, &mut data);
    encode_address(authority, &mut data);
    invoke(&ctx, &data)
}
