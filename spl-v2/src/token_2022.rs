//! Token-2022 CPI helpers and interface re-exports.

extern crate alloc;

use {
    crate::token_shared::multisig_signer_addresses,
    alloc::{string::String, vec::Vec},
    anchor_lang::{require_eq, CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

pub use anchor_lang::programs::Token2022;
pub use spl_token_2022_interface::{
    self as spl_token_2022, extension::ExtensionType, native_mint, ID,
};

pub use crate::token_shared::{
    approve, approve_checked, burn, burn_checked, close_account, freeze_account,
    initialize_account, initialize_account3, initialize_mint, initialize_mint2, mint_to,
    mint_to_checked, revoke, set_authority, sync_native, thaw_account, transfer, transfer_checked,
    Approve, ApproveChecked, Burn, BurnChecked, CloseAccount, FreezeAccount, InitializeAccount,
    InitializeAccount3, InitializeMint, InitializeMint2, MintTo, MintToChecked, Revoke,
    SetAuthority, SyncNative, ThawAccount, Transfer, TransferChecked,
};

#[derive(ToCpiAccounts)]
pub struct GetAccountDataSize<'a> {
    pub mint: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeMintCloseAuthority<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeImmutableOwner<'a> {
    pub account: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct AmountToUiAmount<'a> {
    pub account: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct UiAmountToAmount<'a> {
    pub account: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Reallocate<'a> {
    pub account: CpiHandleMut<'a>,
    #[signer]
    pub payer: CpiHandleMut<'a>,
    pub system_program: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct WithdrawExcessLamports<'a> {
    pub source: CpiHandleMut<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct CreateNativeMint<'a> {
    #[signer]
    pub payer: CpiHandleMut<'a>,
    pub native_mint: CpiHandleMut<'a>,
    pub system_program: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeNonTransferableMint<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct PermanentDelegateInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

fn return_data_from(program: &Address) -> Result<Vec<u8>, ProgramError> {
    let (return_program, data) = anchor_lang::solana_program::program::get_return_data()
        .ok_or(ProgramError::InvalidInstructionData)?;
    require_eq!(
        return_program.to_bytes().as_slice(),
        program.as_ref(),
        ProgramError::IncorrectProgramId
    );
    Ok(data)
}

pub fn get_account_data_size<'a>(
    ctx: CpiContext<'a, GetAccountDataSize<'a>>,
    extension_types: &[ExtensionType],
) -> Result<u64, ProgramError> {
    let ix = spl_token_2022::instruction::get_account_data_size(
        ctx.program,
        ctx.accounts.mint.address(),
        extension_types,
    )?;
    ctx.invoke_ix(ix)?;
    let data = return_data_from(ctx.program)?;
    let bytes: [u8; 8] = data
        .as_slice()
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn initialize_mint_close_authority<'a>(
    ctx: CpiContext<'a, InitializeMintCloseAuthority<'a>>,
    close_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint_close_authority(
        ctx.program,
        ctx.accounts.mint.address(),
        close_authority,
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_immutable_owner<'a>(
    ctx: CpiContext<'a, InitializeImmutableOwner<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_immutable_owner(
        ctx.program,
        ctx.accounts.account.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn amount_to_ui_amount<'a>(
    ctx: CpiContext<'a, AmountToUiAmount<'a>>,
    amount: u64,
) -> Result<String, ProgramError> {
    let ix = spl_token_2022::instruction::amount_to_ui_amount(
        ctx.program,
        ctx.accounts.account.address(),
        amount,
    )?;
    ctx.invoke_ix(ix)?;
    String::from_utf8(return_data_from(ctx.program)?)
        .map_err(|_| ProgramError::InvalidInstructionData)
}

pub fn ui_amount_to_amount<'a>(
    ctx: CpiContext<'a, UiAmountToAmount<'a>>,
    ui_amount: &str,
) -> Result<u64, ProgramError> {
    let ix = spl_token_2022::instruction::ui_amount_to_amount(
        ctx.program,
        ctx.accounts.account.address(),
        ui_amount,
    )?;
    ctx.invoke_ix(ix)?;
    let data = return_data_from(ctx.program)?;
    let bytes: [u8; 8] = data
        .as_slice()
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn reallocate<'a>(
    ctx: CpiContext<'a, Reallocate<'a>>,
    extension_types: &[ExtensionType],
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::reallocate(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.payer.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        extension_types,
    )?;
    ctx.invoke_ix(ix)
}

pub fn withdraw_excess_lamports<'a>(
    ctx: CpiContext<'a, WithdrawExcessLamports<'a>>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::withdraw_excess_lamports(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn create_native_mint<'a>(
    ctx: CpiContext<'a, CreateNativeMint<'a>>,
) -> Result<(), ProgramError> {
    let ix =
        spl_token_2022::instruction::create_native_mint(ctx.program, ctx.accounts.payer.address())?;
    ctx.invoke_ix(ix)
}

pub fn initialize_non_transferable_mint<'a>(
    ctx: CpiContext<'a, InitializeNonTransferableMint<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_non_transferable_mint(
        ctx.program,
        ctx.accounts.mint.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_permanent_delegate<'a>(
    ctx: CpiContext<'a, PermanentDelegateInitialize<'a>>,
    permanent_delegate: &Address,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_permanent_delegate(
        ctx.program,
        ctx.accounts.mint.address(),
        permanent_delegate,
    )?;
    ctx.invoke_ix(ix)
}
