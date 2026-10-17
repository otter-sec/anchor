use {
    super::common::{pubkey_refs, validate_token_2022_program},
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{
        CpiContext, CpiHandle, CpiHandleMut, FixedVec, ToCpiAccounts,
        MAX_RUNTIME_ACCOUNT_METAS,
    },
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

#[derive(ToCpiAccounts)]
pub struct TransferFeeInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TransferFeeSetTransferFee<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TransferCheckedWithFee<'a> {
    pub source: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct HarvestWithheldTokensToMint<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct WithdrawWithheldTokensFromMint<'a> {
    pub mint: CpiHandleMut<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct WithdrawWithheldTokensFromAccounts<'a> {
    pub mint: CpiHandle<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
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
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts)?;
    let ix = spl_token_2022::extension::transfer_fee::instruction::set_transfer_fee(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
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
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts)?;
    let ix = spl_token_2022::extension::transfer_fee::instruction::transfer_checked_with_fee(
        &program,
        ctx.accounts.source.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
        fee,
    )?;
    ctx.invoke_ix(ix)
}

pub fn harvest_withheld_tokens_to_mint<'a>(
    ctx: CpiContext<'a, HarvestWithheldTokensToMint<'a>>,
    sources: &[CpiHandleMut<'a>],
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let mut source_pubkeys = FixedVec::<Pubkey, MAX_RUNTIME_ACCOUNT_METAS>::new();
    for source in sources {
        source_pubkeys
            .try_push(*source.address())
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    let source_refs = pubkey_refs(source_pubkeys.as_slice())?;
    let ix = spl_token_2022::extension::transfer_fee::instruction::harvest_withheld_tokens_to_mint(
        &program,
        ctx.accounts.mint.address(),
        &source_refs,
    )?;
    let mut remaining_accounts = ctx.remaining_accounts.clone();
    for source in sources.iter().copied() {
        remaining_accounts
            .try_push(source.into())
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    ctx.with_remaining_accounts(remaining_accounts)
        .invoke_ix(ix)
}

pub fn withdraw_withheld_tokens_from_mint<'a>(
    ctx: CpiContext<'a, WithdrawWithheldTokensFromMint<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts)?;
    let ix =
        spl_token_2022::extension::transfer_fee::instruction::withdraw_withheld_tokens_from_mint(
            &program,
            ctx.accounts.mint.address(),
            ctx.accounts.destination.address(),
            ctx.accounts.authority.address(),
            &signer_addresses,
        )?;
    ctx.invoke_ix(ix)
}

pub fn withdraw_withheld_tokens_from_accounts<'a>(
    ctx: CpiContext<'a, WithdrawWithheldTokensFromAccounts<'a>>,
    sources: &[CpiHandleMut<'a>],
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts)?;
    let mut source_pubkeys = FixedVec::<Pubkey, MAX_RUNTIME_ACCOUNT_METAS>::new();
    for source in sources {
        source_pubkeys
            .try_push(*source.address())
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    let source_refs = pubkey_refs(source_pubkeys.as_slice())?;
    let ix =
        spl_token_2022::extension::transfer_fee::instruction::withdraw_withheld_tokens_from_accounts(
            &program,
            ctx.accounts.mint.address(),
            ctx.accounts.destination.address(),
            ctx.accounts.authority.address(),
            &signer_addresses,
            &source_refs,
        )?;
    let mut remaining_accounts = ctx.remaining_accounts.clone();
    for source in sources.iter().copied() {
        remaining_accounts
            .try_push(CpiHandle::from(source))
            .map_err(|_| ProgramError::InvalidArgument)?;
    }
    ctx.with_remaining_accounts(remaining_accounts)
        .invoke_ix(ix)
}
