//! Token-2022 CPI helpers and interface re-exports.

extern crate alloc;

pub use anchor_lang::programs::Token2022;
pub(crate) use spl_token_2022_interface as spl_token_2022;
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub use spl_token_2022_interface::{native_mint, ID};
use {
    crate::token_shared::multisig_signer_addresses,
    alloc::vec::Vec,
    anchor_lang::{
        require_eq, CpiContext, CpiHandle, CpiHandleMut, FixedString, FixedVec, ToCpiAccounts,
    },
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[cfg(any(test, target_os = "solana", target_arch = "bpf"))]
const TOKEN_2022_ID: Address =
    anchor_lang::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub const ID: Address = TOKEN_2022_ID;

/// Token-2022 extension identifier without exposing the canonical interface
/// crate's allocator-backed convenience API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ExtensionType {
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

fn canonical_extension_type(value: ExtensionType) -> spl_token_2022::extension::ExtensionType {
    spl_token_2022::extension::ExtensionType::try_from(value as u16)
        .expect("Anchor mirrors canonical Token-2022 extension tags")
}

/// Token account state without exposing the canonical interface crate.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum AccountState {
    #[default]
    Uninitialized = 0,
    Initialized = 1,
    Frozen = 2,
}

pub(crate) fn canonical_account_state(value: AccountState) -> spl_token_2022::state::AccountState {
    spl_token_2022::state::AccountState::try_from(value as u8)
        .expect("Anchor mirrors canonical Token-2022 account states")
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

fn return_data_from(
    program: &Address,
) -> Result<FixedVec<u8, { anchor_lang::solana_program::program::MAX_RETURN_DATA }>, ProgramError> {
    let (return_program, data) = anchor_lang::solana_program::program::get_return_data()
        .ok_or(ProgramError::InvalidInstructionData)?;
    require_eq!(
        return_program.to_bytes().as_slice(),
        program.as_ref(),
        ProgramError::IncorrectProgramId
    );
    let mut output = FixedVec::new();
    output
        .try_extend_from_slice(data.as_slice())
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    Ok(output)
}

pub fn get_account_data_size<'a, const N: usize>(
    ctx: CpiContext<'a, GetAccountDataSize<'a>, N>,
    extension_types: &[ExtensionType],
) -> Result<u64, ProgramError> {
    let extension_types: Vec<_> = extension_types
        .iter()
        .copied()
        .map(canonical_extension_type)
        .collect();
    let ix = spl_token_2022::instruction::get_account_data_size(
        ctx.program,
        ctx.accounts.mint.address(),
        &extension_types,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)?;
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
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn initialize_immutable_owner<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeImmutableOwner<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_immutable_owner(
        ctx.program,
        ctx.accounts.account.address(),
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn amount_to_ui_amount<'a, const N: usize>(
    ctx: CpiContext<'a, AmountToUiAmount<'a>, N>,
    amount: u64,
) -> Result<FixedString<{ anchor_lang::solana_program::program::MAX_RETURN_DATA }>, ProgramError> {
    let ix = spl_token_2022::instruction::amount_to_ui_amount(
        ctx.program,
        ctx.accounts.account.address(),
        amount,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)?;
    parse_ui_amount(return_data_from(ctx.program)?.as_slice())
}

fn parse_ui_amount(
    data: &[u8],
) -> Result<FixedString<{ anchor_lang::solana_program::program::MAX_RETURN_DATA }>, ProgramError> {
    FixedString::try_from(data).map_err(|_| ProgramError::InvalidInstructionData)
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
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)?;
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
    let extension_types: Vec<_> = extension_types
        .iter()
        .copied()
        .map(canonical_extension_type)
        .collect();
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::reallocate(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.payer.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        &extension_types,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
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
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn create_native_mint<'a, const N: usize>(
    ctx: CpiContext<'a, CreateNativeMint<'a>, N>,
) -> Result<(), ProgramError> {
    let ix =
        spl_token_2022::instruction::create_native_mint(ctx.program, ctx.accounts.payer.address())?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn initialize_non_transferable_mint<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeNonTransferableMint<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_non_transferable_mint(
        ctx.program,
        ctx.accounts.mint.address(),
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
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
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_protocol_enums_match_token_2022_wire_values() {
        let extensions = [
            ExtensionType::Uninitialized,
            ExtensionType::TransferFeeConfig,
            ExtensionType::TransferFeeAmount,
            ExtensionType::MintCloseAuthority,
            ExtensionType::ConfidentialTransferMint,
            ExtensionType::ConfidentialTransferAccount,
            ExtensionType::DefaultAccountState,
            ExtensionType::ImmutableOwner,
            ExtensionType::MemoTransfer,
            ExtensionType::NonTransferable,
            ExtensionType::InterestBearingConfig,
            ExtensionType::CpiGuard,
            ExtensionType::PermanentDelegate,
            ExtensionType::NonTransferableAccount,
            ExtensionType::TransferHook,
            ExtensionType::TransferHookAccount,
            ExtensionType::ConfidentialTransferFeeConfig,
            ExtensionType::ConfidentialTransferFeeAmount,
            ExtensionType::MetadataPointer,
            ExtensionType::TokenMetadata,
            ExtensionType::GroupPointer,
            ExtensionType::TokenGroup,
            ExtensionType::GroupMemberPointer,
            ExtensionType::TokenGroupMember,
            ExtensionType::ConfidentialMintBurn,
            ExtensionType::ScaledUiAmount,
            ExtensionType::Pausable,
            ExtensionType::PausableAccount,
        ];
        for extension in extensions {
            assert_eq!(canonical_extension_type(extension) as u16, extension as u16);
        }

        for state in [
            AccountState::Uninitialized,
            AccountState::Initialized,
            AccountState::Frozen,
        ] {
            assert_eq!(canonical_account_state(state) as u8, state as u8);
        }
    }

    #[test]
    fn ui_amount_return_data_must_be_valid_utf8() {
        assert_eq!(parse_ui_amount(b"123.456").unwrap().as_str(), "123.456");
        assert_eq!(
            parse_ui_amount(&[0xff]).unwrap_err(),
            ProgramError::InvalidInstructionData,
        );
    }
}
