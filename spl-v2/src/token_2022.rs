//! Token-2022 CPI helpers and interface re-exports.

pub(crate) use spl_token_2022_interface as spl_token_2022;
use {
    crate::token_shared::multisig_signer_addresses,
    anchor_lang::{
        require_eq, try_extend_instruction_data, CpiContext, CpiHandle, CpiHandleMut, FixedVec,
        InstructionDataBuffer, ToCpiAccounts,
    },
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};
pub use {
    anchor_lang::programs::Token2022,
    spl_token_2022_interface::{native_mint, ID},
};

/// Token-2022 extension identifiers accepted by the V2 CPI helpers.
///
/// This is intentionally an Anchor-owned type rather than a re-export of the
/// canonical SPL enum. The canonical enum exposes allocation-backed helper
/// methods, including methods returning `Vec`, through its public API.
#[repr(u16)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExtensionType {
    /// Uninitialized extension padding.
    #[default]
    Uninitialized = 0,
    TransferFeeConfig = 1,
    TransferFeeAmount = 2,
    MintCloseAuthority = 3,
    ConfidentialTransferMint = 4,
    ConfidentialTransferAccount = 5,
    DefaultAccountState = 6,
    ImmutableOwner = 7,
    MemoTransfer = 8,
    NonTransferable = 9,
    InterestBearingConfig = 10,
    CpiGuard = 11,
    PermanentDelegate = 12,
    NonTransferableAccount = 13,
    TransferHook = 14,
    TransferHookAccount = 15,
    ConfidentialTransferFeeConfig = 16,
    ConfidentialTransferFeeAmount = 17,
    MetadataPointer = 18,
    TokenMetadata = 19,
    GroupPointer = 20,
    TokenGroup = 21,
    GroupMemberPointer = 22,
    TokenGroupMember = 23,
    ConfidentialMintBurn = 24,
    ScaledUiAmount = 25,
    Pausable = 26,
    PausableAccount = 27,
}

impl ExtensionType {
    #[inline(always)]
    fn into_spl(self) -> spl_token_2022::extension::ExtensionType {
        use spl_token_2022::extension::ExtensionType as SplExtensionType;

        match self {
            Self::Uninitialized => SplExtensionType::Uninitialized,
            Self::TransferFeeConfig => SplExtensionType::TransferFeeConfig,
            Self::TransferFeeAmount => SplExtensionType::TransferFeeAmount,
            Self::MintCloseAuthority => SplExtensionType::MintCloseAuthority,
            Self::ConfidentialTransferMint => SplExtensionType::ConfidentialTransferMint,
            Self::ConfidentialTransferAccount => SplExtensionType::ConfidentialTransferAccount,
            Self::DefaultAccountState => SplExtensionType::DefaultAccountState,
            Self::ImmutableOwner => SplExtensionType::ImmutableOwner,
            Self::MemoTransfer => SplExtensionType::MemoTransfer,
            Self::NonTransferable => SplExtensionType::NonTransferable,
            Self::InterestBearingConfig => SplExtensionType::InterestBearingConfig,
            Self::CpiGuard => SplExtensionType::CpiGuard,
            Self::PermanentDelegate => SplExtensionType::PermanentDelegate,
            Self::NonTransferableAccount => SplExtensionType::NonTransferableAccount,
            Self::TransferHook => SplExtensionType::TransferHook,
            Self::TransferHookAccount => SplExtensionType::TransferHookAccount,
            Self::ConfidentialTransferFeeConfig => SplExtensionType::ConfidentialTransferFeeConfig,
            Self::ConfidentialTransferFeeAmount => SplExtensionType::ConfidentialTransferFeeAmount,
            Self::MetadataPointer => SplExtensionType::MetadataPointer,
            Self::TokenMetadata => SplExtensionType::TokenMetadata,
            Self::GroupPointer => SplExtensionType::GroupPointer,
            Self::TokenGroup => SplExtensionType::TokenGroup,
            Self::GroupMemberPointer => SplExtensionType::GroupMemberPointer,
            Self::TokenGroupMember => SplExtensionType::TokenGroupMember,
            Self::ConfidentialMintBurn => SplExtensionType::ConfidentialMintBurn,
            Self::ScaledUiAmount => SplExtensionType::ScaledUiAmount,
            Self::Pausable => SplExtensionType::Pausable,
            Self::PausableAccount => SplExtensionType::PausableAccount,
        }
    }
}

const MAX_EXTENSION_TYPES: usize = 32;

fn into_spl_extension_types(
    extension_types: &[ExtensionType],
) -> Result<FixedVec<spl_token_2022::extension::ExtensionType, MAX_EXTENSION_TYPES>, ProgramError> {
    let mut result = FixedVec::new();
    for extension_type in extension_types {
        result
            .try_push(extension_type.into_spl())
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    Ok(result)
}

/// Default state for a Token-2022 account.
///
/// This small Anchor-owned enum keeps the public extension CPI helpers from
/// exposing the canonical SPL state module. The CPI implementation converts it
/// to the canonical wire value internally.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountState {
    /// The account has not been initialized.
    #[default]
    Uninitialized,
    /// The account is initialized and usable.
    Initialized,
    /// The account is frozen.
    Frozen,
}

impl From<AccountState> for spl_token_2022::state::AccountState {
    fn from(state: AccountState) -> Self {
        match state {
            AccountState::Uninitialized => Self::Uninitialized,
            AccountState::Initialized => Self::Initialized,
            AccountState::Frozen => Self::Frozen,
        }
    }
}

pub use crate::token_shared::{
    approve, approve_checked, burn, burn_checked, close_account, freeze_account,
    initialize_account, initialize_account3, initialize_mint, initialize_mint2, mint_to,
    mint_to_checked, revoke, set_authority, sync_native, thaw_account, transfer, transfer_checked,
    Approve, ApproveChecked, AuthorityType, Burn, BurnChecked, CloseAccount, FreezeAccount,
    InitializeAccount, InitializeAccount3, InitializeMint, InitializeMint2, MintTo, MintToChecked,
    Revoke, SetAuthority, SyncNative, ThawAccount, Transfer, TransferChecked,
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

#[inline(never)]
fn return_data_from(program: &Address) -> Result<alloc::vec::Vec<u8>, ProgramError> {
    let (return_program, data) = anchor_lang::solana_program::program::get_return_data()
        .ok_or(ProgramError::InvalidInstructionData)?;
    require_eq!(
        return_program.to_bytes().as_slice(),
        program.as_ref(),
        ProgramError::IncorrectProgramId
    );
    Ok(data.as_slice().to_vec())
}

pub fn get_account_data_size<'a>(
    ctx: CpiContext<'a, GetAccountDataSize<'a>>,
    extension_types: &[ExtensionType],
) -> Result<u64, ProgramError> {
    let extension_types = into_spl_extension_types(extension_types)?;
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
) -> Result<InstructionDataBuffer, ProgramError> {
    let ix = spl_token_2022::instruction::amount_to_ui_amount(
        ctx.program,
        ctx.accounts.account.address(),
        amount,
    )?;
    ctx.invoke_ix(ix)?;
    let return_data = return_data_from(ctx.program)?;
    let mut output = InstructionDataBuffer::new();
    try_extend_instruction_data(&mut output, return_data.as_slice())?;
    core::str::from_utf8(output.as_slice()).map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(output)
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
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts)?;
    let extension_types = into_spl_extension_types(extension_types)?;
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

pub fn withdraw_excess_lamports<'a>(
    ctx: CpiContext<'a, WithdrawExcessLamports<'a>>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts)?;
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
