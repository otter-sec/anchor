//! Token-2022 extension CPI helpers.

extern crate alloc;

use {
    crate::token_2022::spl_token_2022,
    alloc::{string::String, vec, vec::Vec},
    anchor_lang_v2::{CpiContext, CpiHandle, ToCpiAccounts},
    pinocchio::{address::Address, instruction::InstructionAccount},
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    spl_pod::optional_keys::OptionalNonZeroPubkey,
    spl_token_metadata_interface::state::Field,
};

#[cfg(any(feature = "guardrails", test))]
use anchor_lang_v2::{programs::Token2022, Id};

#[cfg(feature = "guardrails")]
#[inline]
fn validate_token_2022_program(program: &Address) -> Result<(), ProgramError> {
    if *program != Token2022::id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[cfg(not(feature = "guardrails"))]
#[inline]
fn validate_token_2022_program(_program: &Address) -> Result<(), ProgramError> {
    Ok(())
}

fn pubkey_refs(pubkeys: &[Pubkey]) -> Vec<&Pubkey> {
    pubkeys.iter().collect()
}

pub struct CpiGuard<'a> {
    pub account: CpiHandle<'a>,
    pub owner: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for CpiGuard<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::readonly_signer(self.owner.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.owner]
    }
}

pub struct GroupPointerInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for GroupPointerInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct GroupPointerUpdate<'a> {
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for GroupPointerUpdate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            // TODO: Investigate whether v2 should keep mirroring v1's odd
            // single-authority-as-multisig account shape here.
            InstructionAccount::new(self.authority.address(), false, false),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.authority, self.authority]
    }
}

pub struct GroupMemberPointerInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for GroupMemberPointerInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct GroupMemberPointerUpdate<'a> {
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for GroupMemberPointerUpdate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.authority]
    }
}

pub struct PausableInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for PausableInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct PausableToggle<'a> {
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for PausableToggle<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.authority]
    }
}

pub struct TokenMetadataRemoveKey<'a> {
    pub metadata: CpiHandle<'a>,
    pub update_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TokenMetadataRemoveKey<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.metadata.address()),
            InstructionAccount::readonly_signer(self.update_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.metadata, self.update_authority]
    }
}

pub struct DefaultAccountStateInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for DefaultAccountStateInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct DefaultAccountStateUpdate<'a> {
    pub mint: CpiHandle<'a>,
    pub freeze_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for DefaultAccountStateUpdate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.freeze_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.freeze_authority]
    }
}

pub struct ImmutableOwnerInitialize<'a> {
    pub token_account: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for ImmutableOwnerInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.token_account.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.token_account]
    }
}

pub struct InterestBearingMintInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InterestBearingMintInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct InterestBearingMintUpdateRate<'a> {
    pub mint: CpiHandle<'a>,
    pub rate_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for InterestBearingMintUpdateRate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.rate_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.rate_authority]
    }
}

pub struct MemoTransfer<'a> {
    pub account: CpiHandle<'a>,
    pub owner: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for MemoTransfer<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.account.address()),
            InstructionAccount::readonly_signer(self.owner.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.account, self.owner]
    }
}

pub struct MetadataPointerInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for MetadataPointerInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct MetadataPointerUpdate<'a> {
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for MetadataPointerUpdate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.authority]
    }
}

pub struct MintCloseAuthorityInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for MintCloseAuthorityInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct NonTransferableMintInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for NonTransferableMintInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct PermanentDelegateInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for PermanentDelegateInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct TokenMetadataInitialize<'a> {
    pub metadata: CpiHandle<'a>,
    pub update_authority: CpiHandle<'a>,
    pub mint_authority: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TokenMetadataInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.metadata.address()),
            InstructionAccount::new(self.update_authority.address(), false, false),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::readonly_signer(self.mint_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![
            self.metadata,
            self.update_authority,
            self.mint,
            self.mint_authority,
        ]
    }
}

pub struct TokenMetadataUpdateAuthority<'a> {
    pub metadata: CpiHandle<'a>,
    pub current_authority: CpiHandle<'a>,
    pub new_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TokenMetadataUpdateAuthority<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.metadata.address()),
            InstructionAccount::readonly_signer(self.current_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.metadata, self.current_authority]
    }
}

pub struct TokenMetadataUpdateField<'a> {
    pub metadata: CpiHandle<'a>,
    pub update_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TokenMetadataUpdateField<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.metadata.address()),
            InstructionAccount::readonly_signer(self.update_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.metadata, self.update_authority]
    }
}

pub struct TokenGroupInitialize<'a> {
    pub group: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub mint_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TokenGroupInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.group.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::readonly_signer(self.mint_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.group, self.mint, self.mint_authority]
    }
}

pub struct TokenMemberInitialize<'a> {
    pub member: CpiHandle<'a>,
    pub member_mint: CpiHandle<'a>,
    pub member_mint_authority: CpiHandle<'a>,
    pub group: CpiHandle<'a>,
    pub group_update_authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TokenMemberInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.member.address()),
            InstructionAccount::new(self.member_mint.address(), false, false),
            InstructionAccount::readonly_signer(self.member_mint_authority.address()),
            InstructionAccount::writable(self.group.address()),
            InstructionAccount::readonly_signer(self.group_update_authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![
            self.member,
            self.member_mint,
            self.member_mint_authority,
            self.group,
            self.group_update_authority,
        ]
    }
}

pub struct TransferFeeInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferFeeInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct TransferFeeSetTransferFee<'a> {
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferFeeSetTransferFee<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.authority]
    }
}

pub struct TransferCheckedWithFee<'a> {
    pub source: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    pub destination: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferCheckedWithFee<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.source.address()),
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::writable(self.destination.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.source, self.mint, self.destination, self.authority]
    }
}

pub struct HarvestWithheldTokensToMint<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for HarvestWithheldTokensToMint<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct WithdrawWithheldTokensFromMint<'a> {
    pub mint: CpiHandle<'a>,
    pub destination: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for WithdrawWithheldTokensFromMint<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::writable(self.destination.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.destination, self.authority]
    }
}

pub struct WithdrawWithheldTokensFromAccounts<'a> {
    pub mint: CpiHandle<'a>,
    pub destination: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for WithdrawWithheldTokensFromAccounts<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::new(self.mint.address(), false, false),
            InstructionAccount::writable(self.destination.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.destination, self.authority]
    }
}

pub struct TransferHookInitialize<'a> {
    pub mint: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferHookInitialize<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![InstructionAccount::writable(self.mint.address())]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint]
    }
}

pub struct TransferHookUpdate<'a> {
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
}

impl<'a> ToCpiAccounts<'a> for TransferHookUpdate<'a> {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        vec![
            InstructionAccount::writable(self.mint.address()),
            InstructionAccount::readonly_signer(self.authority.address()),
        ]
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        vec![self.mint, self.authority]
    }
}

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked."
)]
pub fn cpi_guard_enable<'a>(ctx: CpiContext<'a, CpiGuard<'a>>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::cpi_guard::instruction::enable_cpi_guard(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.owner.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked."
)]
pub fn cpi_guard_disable<'a>(ctx: CpiContext<'a, CpiGuard<'a>>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::cpi_guard::instruction::disable_cpi_guard(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.owner.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn group_pointer_initialize<'a>(
    ctx: CpiContext<'a, GroupPointerInitialize<'a>>,
    authority: Option<&Address>,
    group_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::group_pointer::instruction::initialize(
        ctx.program,
        ctx.accounts.mint.address(),
        authority.copied(),
        group_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn group_pointer_update<'a>(
    ctx: CpiContext<'a, GroupPointerUpdate<'a>>,
    group_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::group_pointer::instruction::update(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[ctx.accounts.authority.address()],
        group_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn group_member_pointer_initialize<'a>(
    ctx: CpiContext<'a, GroupMemberPointerInitialize<'a>>,
    authority: Option<&Address>,
    member_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::group_member_pointer::instruction::initialize(
        ctx.program,
        ctx.accounts.mint.address(),
        authority.copied(),
        member_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn group_member_pointer_update<'a>(
    ctx: CpiContext<'a, GroupMemberPointerUpdate<'a>>,
    member_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::group_member_pointer::instruction::update(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        member_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn pausable_initialize<'a>(
    ctx: CpiContext<'a, PausableInitialize<'a>>,
    authority: &Address,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::pausable::instruction::initialize(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
    )?;
    ctx.invoke_ix(ix)
}

pub fn pausable_pause<'a>(ctx: CpiContext<'a, PausableToggle<'a>>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::pausable::instruction::pause(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn pausable_resume<'a>(ctx: CpiContext<'a, PausableToggle<'a>>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::pausable::instruction::resume(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn token_metadata_remove_key<'a>(
    ctx: CpiContext<'a, TokenMetadataRemoveKey<'a>>,
    key: String,
    idempotent: bool,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::remove_key(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        key,
        idempotent,
    );
    ctx.invoke_ix(ix)
}

pub fn default_account_state_initialize<'a>(
    ctx: CpiContext<'a, DefaultAccountStateInitialize<'a>>,
    state: &spl_token_2022::state::AccountState,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix =
        spl_token_2022::extension::default_account_state::instruction::initialize_default_account_state(
            &program,
            ctx.accounts.mint.address(),
            state,
        )?;
    ctx.invoke_ix(ix)
}

pub fn default_account_state_update<'a>(
    ctx: CpiContext<'a, DefaultAccountStateUpdate<'a>>,
    state: &spl_token_2022::state::AccountState,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix =
        spl_token_2022::extension::default_account_state::instruction::update_default_account_state(
            &program,
            ctx.accounts.mint.address(),
            ctx.accounts.freeze_authority.address(),
            &[],
            state,
        )?;
    ctx.invoke_ix(ix)
}

pub fn immutable_owner_initialize<'a>(
    ctx: CpiContext<'a, ImmutableOwnerInitialize<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::instruction::initialize_immutable_owner(
        &program,
        ctx.accounts.token_account.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn interest_bearing_mint_initialize<'a>(
    ctx: CpiContext<'a, InterestBearingMintInitialize<'a>>,
    rate_authority: Option<&Address>,
    rate: i16,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::interest_bearing_mint::instruction::initialize(
        &program,
        ctx.accounts.mint.address(),
        rate_authority.copied(),
        rate,
    )?;
    ctx.invoke_ix(ix)
}

pub fn interest_bearing_mint_update_rate<'a>(
    ctx: CpiContext<'a, InterestBearingMintUpdateRate<'a>>,
    rate: i16,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::interest_bearing_mint::instruction::update_rate(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.rate_authority.address(),
        &[],
        rate,
    )?;
    ctx.invoke_ix(ix)
}

pub fn memo_transfer_initialize<'a>(
    ctx: CpiContext<'a, MemoTransfer<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::memo_transfer::instruction::enable_required_transfer_memos(
        &program,
        ctx.accounts.account.address(),
        ctx.accounts.owner.address(),
        &[],
    )?;
    ctx.invoke_ix(ix)
}

pub fn memo_transfer_disable<'a>(
    ctx: CpiContext<'a, MemoTransfer<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix =
        spl_token_2022::extension::memo_transfer::instruction::disable_required_transfer_memos(
            &program,
            ctx.accounts.account.address(),
            ctx.accounts.owner.address(),
            &[],
        )?;
    ctx.invoke_ix(ix)
}

pub fn metadata_pointer_initialize<'a>(
    ctx: CpiContext<'a, MetadataPointerInitialize<'a>>,
    authority: Option<&Address>,
    metadata_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::metadata_pointer::instruction::initialize(
        &program,
        ctx.accounts.mint.address(),
        authority.copied(),
        metadata_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn metadata_pointer_update<'a>(
    ctx: CpiContext<'a, MetadataPointerUpdate<'a>>,
    metadata_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::metadata_pointer::instruction::update(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        metadata_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn mint_close_authority_initialize<'a>(
    ctx: CpiContext<'a, MintCloseAuthorityInitialize<'a>>,
    close_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let close_authority = close_authority.copied();
    let ix = spl_token_2022::instruction::initialize_mint_close_authority(
        &program,
        ctx.accounts.mint.address(),
        close_authority.as_ref(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn non_transferable_mint_initialize<'a>(
    ctx: CpiContext<'a, NonTransferableMintInitialize<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::instruction::initialize_non_transferable_mint(
        &program,
        ctx.accounts.mint.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn permanent_delegate_initialize<'a>(
    ctx: CpiContext<'a, PermanentDelegateInitialize<'a>>,
    permanent_delegate: &Address,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::instruction::initialize_permanent_delegate(
        &program,
        ctx.accounts.mint.address(),
        permanent_delegate,
    )?;
    ctx.invoke_ix(ix)
}

pub fn token_metadata_initialize<'a>(
    ctx: CpiContext<'a, TokenMetadataInitialize<'a>>,
    name: String,
    symbol: String,
    uri: String,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::initialize(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.mint_authority.address(),
        name,
        symbol,
        uri,
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_update_authority<'a>(
    ctx: CpiContext<'a, TokenMetadataUpdateAuthority<'a>>,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let new_authority = OptionalNonZeroPubkey::try_from(new_authority.copied())
        .map_err(|_| ProgramError::InvalidArgument)?;
    let ix = spl_token_metadata_interface::instruction::update_authority(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.current_authority.address(),
        new_authority,
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_update_field<'a>(
    ctx: CpiContext<'a, TokenMetadataUpdateField<'a>>,
    field: Field,
    value: String,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::update_field(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        field,
        value,
    );
    ctx.invoke_ix(ix)
}

pub fn token_group_initialize<'a>(
    ctx: CpiContext<'a, TokenGroupInitialize<'a>>,
    update_authority: Option<&Address>,
    max_size: u64,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_group_interface::instruction::initialize_group(
        &program,
        ctx.accounts.group.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.mint_authority.address(),
        update_authority.copied(),
        max_size,
    );
    ctx.invoke_ix(ix)
}

pub fn token_member_initialize<'a>(
    ctx: CpiContext<'a, TokenMemberInitialize<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_group_interface::instruction::initialize_member(
        &program,
        ctx.accounts.member.address(),
        ctx.accounts.member_mint.address(),
        ctx.accounts.member_mint_authority.address(),
        ctx.accounts.group.address(),
        ctx.accounts.group_update_authority.address(),
    );
    ctx.invoke_ix(ix)
}

pub fn transfer_fee_initialize<'a>(
    ctx: CpiContext<'a, TransferFeeInitialize<'a>>,
    transfer_fee_config_authority: Option<&Address>,
    withdraw_withheld_authority: Option<&Address>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let transfer_fee_config_authority = transfer_fee_config_authority.copied();
    let withdraw_withheld_authority = withdraw_withheld_authority.copied();
    let ix = spl_token_2022::extension::transfer_fee::instruction::initialize_transfer_fee_config(
        &program,
        ctx.accounts.mint.address(),
        transfer_fee_config_authority.as_ref(),
        withdraw_withheld_authority.as_ref(),
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_fee_set<'a>(
    ctx: CpiContext<'a, TransferFeeSetTransferFee<'a>>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::transfer_fee::instruction::set_transfer_fee(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_checked_with_fee<'a>(
    ctx: CpiContext<'a, TransferCheckedWithFee<'a>>,
    amount: u64,
    decimals: u8,
    fee: u64,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::transfer_fee::instruction::transfer_checked_with_fee(
        &program,
        ctx.accounts.source.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
        fee,
    )?;
    ctx.invoke_ix(ix)
}

pub fn harvest_withheld_tokens_to_mint<'a>(
    ctx: CpiContext<'a, HarvestWithheldTokensToMint<'a>>,
    sources: Vec<CpiHandle<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let source_pubkeys: Vec<Pubkey> = sources.iter().map(|source| *source.address()).collect();
    let source_refs = pubkey_refs(&source_pubkeys);
    let ix = spl_token_2022::extension::transfer_fee::instruction::harvest_withheld_tokens_to_mint(
        &program,
        ctx.accounts.mint.address(),
        &source_refs,
    )?;
    let mut remaining_accounts = sources;
    remaining_accounts.extend(ctx.remaining_accounts.iter().copied());
    ctx.with_remaining_accounts(remaining_accounts)
        .invoke_ix(ix)
}

pub fn withdraw_withheld_tokens_from_mint<'a>(
    ctx: CpiContext<'a, WithdrawWithheldTokensFromMint<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix =
        spl_token_2022::extension::transfer_fee::instruction::withdraw_withheld_tokens_from_mint(
            &program,
            ctx.accounts.mint.address(),
            ctx.accounts.destination.address(),
            ctx.accounts.authority.address(),
            &[],
        )?;
    ctx.invoke_ix(ix)
}

pub fn withdraw_withheld_tokens_from_accounts<'a>(
    ctx: CpiContext<'a, WithdrawWithheldTokensFromAccounts<'a>>,
    sources: Vec<CpiHandle<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let source_pubkeys: Vec<Pubkey> = sources.iter().map(|source| *source.address()).collect();
    let source_refs = pubkey_refs(&source_pubkeys);
    let ix =
        spl_token_2022::extension::transfer_fee::instruction::withdraw_withheld_tokens_from_accounts(
            &program,
            ctx.accounts.mint.address(),
            ctx.accounts.destination.address(),
            ctx.accounts.authority.address(),
            &[],
            &source_refs,
        )?;
    let mut remaining_accounts = sources;
    remaining_accounts.extend(ctx.remaining_accounts.iter().copied());
    ctx.with_remaining_accounts(remaining_accounts)
        .invoke_ix(ix)
}

pub fn transfer_hook_initialize<'a>(
    ctx: CpiContext<'a, TransferHookInitialize<'a>>,
    authority: Option<&Address>,
    transfer_hook_program_id: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::transfer_hook::instruction::initialize(
        &program,
        ctx.accounts.mint.address(),
        authority.copied(),
        transfer_hook_program_id.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_hook_update<'a>(
    ctx: CpiContext<'a, TransferHookUpdate<'a>>,
    transfer_hook_program_id: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::transfer_hook::instruction::update(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        transfer_hook_program_id.copied(),
    )?;
    ctx.invoke_ix(ix)
}

#[allow(deprecated)]
pub mod cpi_guard {
    pub use super::{cpi_guard_disable, cpi_guard_enable, CpiGuard};
}

pub mod confidential_transfer {}

pub mod confidential_transfer_fee {}

pub mod default_account_state {
    pub use super::{
        default_account_state_initialize, default_account_state_update,
        DefaultAccountStateInitialize, DefaultAccountStateUpdate,
    };
}

pub mod group_pointer {
    pub use super::{
        group_pointer_initialize, group_pointer_update, GroupPointerInitialize, GroupPointerUpdate,
    };
}

pub mod group_member_pointer {
    pub use super::{
        group_member_pointer_initialize, group_member_pointer_update, GroupMemberPointerInitialize,
        GroupMemberPointerUpdate,
    };
}

pub mod immutable_owner {
    pub use super::{immutable_owner_initialize, ImmutableOwnerInitialize};
}

pub mod interest_bearing_mint {
    pub use super::{
        interest_bearing_mint_initialize, interest_bearing_mint_update_rate,
        InterestBearingMintInitialize, InterestBearingMintUpdateRate,
    };
}

pub mod memo_transfer {
    pub use super::{memo_transfer_disable, memo_transfer_initialize, MemoTransfer};
}

pub mod metadata_pointer {
    pub use super::{
        metadata_pointer_initialize, metadata_pointer_update, MetadataPointerInitialize,
        MetadataPointerUpdate,
    };
}

pub mod mint_close_authority {
    pub use super::{mint_close_authority_initialize, MintCloseAuthorityInitialize};
}

pub mod non_transferable {
    pub use super::{non_transferable_mint_initialize, NonTransferableMintInitialize};
}

pub mod pausable {
    pub use super::{
        pausable_initialize, pausable_pause, pausable_resume, PausableInitialize, PausableToggle,
    };
}

pub mod permanent_delegate {
    pub use super::{permanent_delegate_initialize, PermanentDelegateInitialize};
}

pub mod token_group {
    pub use super::{
        token_group_initialize, token_member_initialize, TokenGroupInitialize,
        TokenMemberInitialize,
    };
}

pub mod token_metadata {
    pub use super::{
        token_metadata_initialize, token_metadata_remove_key, token_metadata_update_authority,
        token_metadata_update_field, TokenMetadataInitialize, TokenMetadataRemoveKey,
        TokenMetadataUpdateAuthority, TokenMetadataUpdateField,
    };
    pub use spl_token_metadata_interface::state::Field;
}

pub mod transfer_fee {
    pub use super::{
        harvest_withheld_tokens_to_mint, transfer_checked_with_fee, transfer_fee_initialize,
        transfer_fee_set, withdraw_withheld_tokens_from_accounts,
        withdraw_withheld_tokens_from_mint, HarvestWithheldTokensToMint, TransferCheckedWithFee,
        TransferFeeInitialize, TransferFeeSetTransferFee, WithdrawWithheldTokensFromAccounts,
        WithdrawWithheldTokensFromMint,
    };
}

pub mod transfer_hook {
    pub use super::{
        transfer_hook_initialize, transfer_hook_update, TransferHookInitialize, TransferHookUpdate,
    };
}

pub use {spl_pod, spl_token_metadata_interface};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_2022_program_check_accepts_canonical_id() {
        assert_eq!(validate_token_2022_program(&Token2022::id()), Ok(()));
    }

    #[test]
    #[cfg(feature = "guardrails")]
    fn token_2022_program_check_rejects_other_programs() {
        assert_eq!(
            validate_token_2022_program(&Address::new_from_array([1; 32])),
            Err(ProgramError::IncorrectProgramId)
        );
    }
}
