use {
    super::common::validate_token_2022_program,
    crate::token_2022::spl_token_2022,
    anchor_lang::{CpiContext, CpiHandleMut, ToCpiAccounts},
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct NonTransferableMintInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
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
