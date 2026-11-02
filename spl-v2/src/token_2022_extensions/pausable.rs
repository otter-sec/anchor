use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::add_signers},
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
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
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
    let mut ix = spl_token_2022::extension::pausable::instruction::pause(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn pausable_resume<'a>(ctx: CpiContext<'a, PausableToggle<'a>>) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let mut ix = spl_token_2022::extension::pausable::instruction::resume(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}
