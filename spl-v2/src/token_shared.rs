//! Shared base token CPI helpers used by `token` and `token_2022`.
//!
//! Authority-bearing helpers support both single authorities and SPL
//! multisigs. For a multisig, pass the member signer handles in canonical
//! order through [`CpiContext::with_remaining_accounts`].

extern crate alloc;

#[cfg(feature = "guardrails")]
use anchor_lang::{require, Id};
use {
    alloc::vec::Vec,
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    spl_token_2022_interface as spl_token_2022,
};

/// SPL Token encodes a multisig authority by marking the authority account
/// non-signer and appending each member signer to the instruction. Token CPI
/// callers provide those member handles through `remaining_accounts`.
pub(crate) fn multisig_signer_addresses<'a>(accounts: &[CpiHandle<'a>]) -> Vec<&'a Address> {
    accounts.iter().map(CpiHandle::address).collect()
}

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
///   2. `[signer]` authority, or `[]` multisig authority followed by member signers
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
///   3. `[signer]` authority, or `[]` multisig authority followed by member signers
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
    ctx: CpiContext<'a, InitializeAccount<'a>>,
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
    ctx: CpiContext<'a, InitializeAccount3<'a>>,
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
    ctx: CpiContext<'a, InitializeMint<'a>>,
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
    ctx: CpiContext<'a, InitializeMint2<'a>>,
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

pub fn transfer<'a>(ctx: CpiContext<'a, Transfer<'a>>, amount: u64) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    #[allow(deprecated)]
    let ix = spl_token_2022::instruction::transfer(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_checked<'a>(
    ctx: CpiContext<'a, TransferChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::transfer_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_to<'a>(ctx: CpiContext<'a, MintTo<'a>>, amount: u64) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::mint_to(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_to_checked<'a>(
    ctx: CpiContext<'a, MintToChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::mint_to_checked(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn burn<'a>(ctx: CpiContext<'a, Burn<'a>>, amount: u64) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::burn(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn burn_checked<'a>(
    ctx: CpiContext<'a, BurnChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::burn_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn approve<'a>(ctx: CpiContext<'a, Approve<'a>>, amount: u64) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::approve(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    ctx.invoke_ix(ix)
}

pub fn approve_checked<'a>(
    ctx: CpiContext<'a, ApproveChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::approve_checked(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn revoke<'a>(ctx: CpiContext<'a, Revoke<'a>>) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::revoke(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn set_authority<'a>(
    ctx: CpiContext<'a, SetAuthority<'a>>,
    authority_type: spl_token_2022::instruction::AuthorityType,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::set_authority(
        ctx.program,
        ctx.accounts.account_or_mint.address(),
        new_authority,
        authority_type,
        ctx.accounts.current_authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn close_account<'a>(ctx: CpiContext<'a, CloseAccount<'a>>) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::close_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn freeze_account<'a>(ctx: CpiContext<'a, FreezeAccount<'a>>) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::freeze_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn thaw_account<'a>(ctx: CpiContext<'a, ThawAccount<'a>>) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::thaw_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn sync_native<'a>(ctx: CpiContext<'a, SyncNative<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::sync_native(ctx.program, ctx.accounts.account.address())?;
    ctx.invoke_ix(ix)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        anchor_lang::{
            programs::Token,
            testing::{AccountBuffer, MIN_ACCOUNT_BUF},
            Id,
        },
    };

    fn signer(address: [u8; 32]) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
        let buffer = AccountBuffer::new();
        buffer.init(address, [9; 32], 8, true, false, false);
        buffer
    }

    #[test]
    fn remaining_signers_encode_canonical_multisig_layout() {
        let member_one = signer([4; 32]);
        let member_two = signer([5; 32]);
        let member_one_view = unsafe { member_one.view() };
        let member_two_view = unsafe { member_two.view() };
        let handles = [
            CpiHandle::readonly(&member_one_view),
            CpiHandle::readonly(&member_two_view),
        ];
        let signer_addresses = multisig_signer_addresses(&handles);

        #[allow(deprecated)]
        let ix = spl_token_2022::instruction::transfer(
            &Token::id(),
            &Address::new_from_array([1; 32]),
            &Address::new_from_array([2; 32]),
            &Address::new_from_array([3; 32]),
            &signer_addresses,
            7,
        )
        .unwrap();

        assert_eq!(ix.accounts.len(), 5);
        assert!(
            !ix.accounts[2].is_signer,
            "multisig account is not a signer"
        );
        assert!(ix.accounts[3].is_signer);
        assert!(ix.accounts[4].is_signer);
        assert_eq!(ix.accounts[3].pubkey.as_ref(), [4; 32].as_slice());
        assert_eq!(ix.accounts[4].pubkey.as_ref(), [5; 32].as_slice());
    }
}
