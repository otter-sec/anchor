//! Token-2022 CPI helpers and interface re-exports.

extern crate alloc;

use {
    crate::token_shared::multisig_signer_addresses,
    anchor_lang::{
        require_eq, CpiContext, CpiHandle, CpiHandleMut, FixedVec, InstructionDataBuffer,
        ToCpiAccounts,
    },
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

pub use anchor_lang::programs::Token2022;
pub use spl_token_2022_interface::{native_mint, ID};
pub use spl_token_2022_interface::state::AccountState;

pub(crate) use spl_token_2022_interface as spl_token_2022;

/// Token-2022 extension identifiers exposed by Anchor's allocator-neutral API.
///
/// The canonical SPL interface crate is kept behind this module boundary
/// because some of its convenience methods return heap-backed collections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ExtensionType {
    Uninitialized,
    TransferFeeConfig,
    TransferFeeAmount,
    MintCloseAuthority,
    ConfidentialTransferMint,
    ConfidentialTransferAccount,
    DefaultAccountState,
    ImmutableOwner,
    MemoTransfer,
    NonTransferable,
    InterestBearingConfig,
    CpiGuard,
    PermanentDelegate,
    NonTransferableAccount,
    TransferHook,
    TransferHookAccount,
    ConfidentialTransferFeeConfig,
    ConfidentialTransferFeeAmount,
    MetadataPointer,
    TokenMetadata,
    GroupPointer,
    TokenGroup,
    GroupMemberPointer,
    TokenGroupMember,
    ConfidentialMintBurn,
    ScaledUiAmount,
    Pausable,
    PausableAccount,
}

const MAX_EXTENSION_TYPES: usize = 32;

impl From<ExtensionType> for spl_token_2022_interface::extension::ExtensionType {
    fn from(extension_type: ExtensionType) -> Self {
        use spl_token_2022_interface::extension::ExtensionType as Canonical;

        match extension_type {
            ExtensionType::Uninitialized => Canonical::Uninitialized,
            ExtensionType::TransferFeeConfig => Canonical::TransferFeeConfig,
            ExtensionType::TransferFeeAmount => Canonical::TransferFeeAmount,
            ExtensionType::MintCloseAuthority => Canonical::MintCloseAuthority,
            ExtensionType::ConfidentialTransferMint => Canonical::ConfidentialTransferMint,
            ExtensionType::ConfidentialTransferAccount => Canonical::ConfidentialTransferAccount,
            ExtensionType::DefaultAccountState => Canonical::DefaultAccountState,
            ExtensionType::ImmutableOwner => Canonical::ImmutableOwner,
            ExtensionType::MemoTransfer => Canonical::MemoTransfer,
            ExtensionType::NonTransferable => Canonical::NonTransferable,
            ExtensionType::InterestBearingConfig => Canonical::InterestBearingConfig,
            ExtensionType::CpiGuard => Canonical::CpiGuard,
            ExtensionType::PermanentDelegate => Canonical::PermanentDelegate,
            ExtensionType::NonTransferableAccount => Canonical::NonTransferableAccount,
            ExtensionType::TransferHook => Canonical::TransferHook,
            ExtensionType::TransferHookAccount => Canonical::TransferHookAccount,
            ExtensionType::ConfidentialTransferFeeConfig => Canonical::ConfidentialTransferFeeConfig,
            ExtensionType::ConfidentialTransferFeeAmount => Canonical::ConfidentialTransferFeeAmount,
            ExtensionType::MetadataPointer => Canonical::MetadataPointer,
            ExtensionType::TokenMetadata => Canonical::TokenMetadata,
            ExtensionType::GroupPointer => Canonical::GroupPointer,
            ExtensionType::TokenGroup => Canonical::TokenGroup,
            ExtensionType::GroupMemberPointer => Canonical::GroupMemberPointer,
            ExtensionType::TokenGroupMember => Canonical::TokenGroupMember,
            ExtensionType::ConfidentialMintBurn => Canonical::ConfidentialMintBurn,
            ExtensionType::ScaledUiAmount => Canonical::ScaledUiAmount,
            ExtensionType::Pausable => Canonical::Pausable,
            ExtensionType::PausableAccount => Canonical::PausableAccount,
        }
    }
}

fn canonical_extension_types(
    extension_types: &[ExtensionType],
) -> Result<FixedVec<spl_token_2022_interface::extension::ExtensionType, MAX_EXTENSION_TYPES>, ProgramError> {
    let mut canonical = FixedVec::new();
    for extension_type in extension_types {
        canonical
            .try_push((*extension_type).into())
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    Ok(canonical)
}

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

fn return_data_from(program: &Address) -> Result<InstructionDataBuffer, ProgramError> {
    let (return_program, data) = anchor_lang::solana_program::program::get_return_data()
        .ok_or(ProgramError::InvalidInstructionData)?;
    require_eq!(
        return_program.to_bytes().as_slice(),
        program.as_ref(),
        ProgramError::IncorrectProgramId
    );
    let mut output = InstructionDataBuffer::new();
    output
        .try_extend_from_slice(data.as_slice())
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(output)
}

pub fn get_account_data_size<'a, const N: usize>(
    ctx: CpiContext<'a, GetAccountDataSize<'a>, N>,
    extension_types: &[ExtensionType],
) -> Result<u64, ProgramError> {
    let extension_types = canonical_extension_types(extension_types)?;
    let ix = spl_token_2022::instruction::get_account_data_size(
        ctx.program,
        ctx.accounts.mint.address(),
        extension_types.as_slice(),
    )?;
    ctx.invoke_ix(ix)?;
    let data = return_data_from(ctx.program)?;
    let bytes: [u8; 8] = data
        .as_slice()
        .try_into()
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn initialize_mint_close_authority<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeMintCloseAuthority<'a>, N>,
    close_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint_close_authority(
        ctx.program,
        ctx.accounts.mint.address(),
        close_authority,
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_immutable_owner<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeImmutableOwner<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_immutable_owner(
        ctx.program,
        ctx.accounts.account.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn amount_to_ui_amount<'a, const N: usize>(
    ctx: CpiContext<'a, AmountToUiAmount<'a>, N>,
    amount: u64,
) -> Result<InstructionDataBuffer, ProgramError> {
    let ix = spl_token_2022::instruction::amount_to_ui_amount(
        ctx.program,
        ctx.accounts.account.address(),
        amount,
    )?;
    ctx.invoke_ix(ix)?;
    Ok(return_data_from(ctx.program)?)
}

pub fn ui_amount_to_amount<'a, const N: usize>(
    ctx: CpiContext<'a, UiAmountToAmount<'a>, N>,
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

pub fn reallocate<'a, const N: usize>(
    ctx: CpiContext<'a, Reallocate<'a>, N>,
    extension_types: &[ExtensionType],
) -> Result<(), ProgramError> {
    let extension_types = canonical_extension_types(extension_types)?;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::reallocate(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.payer.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        extension_types.as_slice(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn withdraw_excess_lamports<'a, const N: usize>(
    ctx: CpiContext<'a, WithdrawExcessLamports<'a>, N>,
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

pub fn create_native_mint<'a, const N: usize>(
    ctx: CpiContext<'a, CreateNativeMint<'a>, N>,
) -> Result<(), ProgramError> {
    let ix =
        spl_token_2022::instruction::create_native_mint(ctx.program, ctx.accounts.payer.address())?;
    ctx.invoke_ix(ix)
}

pub fn initialize_non_transferable_mint<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeNonTransferableMint<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_non_transferable_mint(
        ctx.program,
        ctx.accounts.mint.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_permanent_delegate<'a, const N: usize>(
    ctx: CpiContext<'a, PermanentDelegateInitialize<'a>, N>,
    permanent_delegate: &Address,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_permanent_delegate(
        ctx.program,
        ctx.accounts.mint.address(),
        permanent_delegate,
    )?;
    ctx.invoke_ix(ix)
}
