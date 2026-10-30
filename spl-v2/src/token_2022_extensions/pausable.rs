use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct PausableInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct PausableToggle<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

pub fn pausable_initialize<'a, const N: usize>(
    ctx: CpiContext<'a, PausableInitialize<'a>, N>,
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

pub fn pausable_pause<'a, const N: usize>(ctx: CpiContext<'a, PausableToggle<'a>, N>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::extension::pausable::instruction::pause(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn pausable_resume<'a, const N: usize>(ctx: CpiContext<'a, PausableToggle<'a>, N>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::extension::pausable::instruction::resume(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}
