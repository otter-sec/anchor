use {
    super::common::validate_token_2022_program,
    crate::token_2022::spl_token_2022,
    anchor_lang::{CpiContext, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct MintCloseAuthorityInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
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
