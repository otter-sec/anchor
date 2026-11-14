//! Shared base token CPI helpers used by `token` and `token_2022`.

extern crate alloc;

#[cfg(feature = "guardrails")]
use anchor_lang::{require, Id};
use {
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    spl_token_2022_interface as spl_token_2022,
};

#[cfg(feature = "guardrails")]
#[inline]
pub(crate) fn validate_token_interface_program(program_id: &Address) -> Result<(), ProgramError> {
    require!(
        anchor_lang::address_eq(program_id, &anchor_lang::programs::Token::id())
            || anchor_lang::address_eq(program_id, &anchor_lang::programs::Token2022::id()),
        ProgramError::IncorrectProgramId
    );
    Ok(())
}

#[cfg(not(feature = "guardrails"))]
#[inline]
pub(crate) fn validate_token_interface_program(_program_id: &Address) -> Result<(), ProgramError> {
    Ok(())
}

#[derive(ToCpiAccounts)]
pub struct InitializeAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeAccount3<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[account_meta(skip)]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeMint<'a> {
    pub mint: CpiHandleMut<'a>,
    pub rent: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeMint2<'a> {
    pub mint: CpiHandleMut<'a>,
}

/// Token / Token-2022 transfer instruction — accounts list:
///   0. `[writable]` from
///   1. `[writable]` to
///   2. `[signer]` authority (owner/delegate)
#[derive(ToCpiAccounts)]
pub struct Transfer<'a> {
    pub from: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

/// Token / Token-2022 checked transfer instruction — adds the mint and verifies
/// the declared decimals match on-chain.
///   0. `[writable]` from
///   1. `[]` mint
///   2. `[writable]` to
///   3. `[signer]` authority
#[derive(ToCpiAccounts)]
pub struct TransferChecked<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct MintTo<'a> {
    pub mint: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct MintToChecked<'a> {
    pub mint: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Burn<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct BurnChecked<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Approve<'a> {
    pub to: CpiHandleMut<'a>,
    pub delegate: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct ApproveChecked<'a> {
    pub to: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub delegate: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Revoke<'a> {
    pub source: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct SetAuthority<'a> {
    pub account_or_mint: CpiHandleMut<'a>,
    #[signer]
    pub current_authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct CloseAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct FreezeAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct ThawAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct SyncNative<'a> {
    pub account: CpiHandleMut<'a>,
}

pub fn initialize_account<'a>(
    ctx: CpiContext<'a, InitializeAccount<'a>, 4>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_account3<'a>(
    ctx: CpiContext<'a, InitializeAccount3<'a>, 3>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account3(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_mint<'a>(
    ctx: CpiContext<'a, InitializeMint<'a>, 2>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_mint2<'a>(
    ctx: CpiContext<'a, InitializeMint2<'a>, 1>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint2(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer<'a, const N: usize>(
    ctx: CpiContext<'a, Transfer<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    #[allow(deprecated)]
    let ix = spl_token_2022::instruction::transfer(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_checked<'a, const N: usize>(
    ctx: CpiContext<'a, TransferChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::transfer_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_to<'a, const N: usize>(
    ctx: CpiContext<'a, MintTo<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::mint_to(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_to_checked<'a, const N: usize>(
    ctx: CpiContext<'a, MintToChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::mint_to_checked(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn burn<'a, const N: usize>(
    ctx: CpiContext<'a, Burn<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::burn(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn burn_checked<'a, const N: usize>(
    ctx: CpiContext<'a, BurnChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::burn_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn approve<'a, const N: usize>(
    ctx: CpiContext<'a, Approve<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::approve(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn approve_checked<'a, const N: usize>(
    ctx: CpiContext<'a, ApproveChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::approve_checked(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn revoke<'a, const N: usize>(ctx: CpiContext<'a, Revoke<'a>, N>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::revoke(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn set_authority<'a, const N: usize>(
    ctx: CpiContext<'a, SetAuthority<'a>, N>,
    authority_type: spl_token_2022::instruction::AuthorityType,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::set_authority(
        ctx.program,
        ctx.accounts.account_or_mint.address(),
        new_authority,
        authority_type,
        ctx.accounts.current_authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn close_account<'a, const N: usize>(
    ctx: CpiContext<'a, CloseAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::close_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn freeze_account<'a, const N: usize>(
    ctx: CpiContext<'a, FreezeAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::freeze_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn thaw_account<'a, const N: usize>(
    ctx: CpiContext<'a, ThawAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::thaw_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn sync_native<'a>(
    ctx: CpiContext<'a, SyncNative<'a>, 1>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::sync_native(ctx.program, ctx.accounts.account.address())?;
    ctx.invoke_ix(ix)
}
