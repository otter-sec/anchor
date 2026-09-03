use {
    super::common::validate_token_2022_program,
    crate::token_2022::spl_token_2022,
    anchor_lang::{CpiContext, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct PermanentDelegateInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
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
