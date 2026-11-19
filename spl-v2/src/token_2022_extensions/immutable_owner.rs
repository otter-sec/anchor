use {
    super::common::validate_token_2022_program,
    crate::token_2022::spl_token_2022,
    anchor_lang::{CpiContext, CpiHandleMut, ToCpiAccounts},
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct ImmutableOwnerInitialize<'a> {
    pub token_account: CpiHandleMut<'a>,
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
