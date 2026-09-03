use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct InterestBearingMintInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InterestBearingMintUpdateRate<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub rate_authority: CpiHandle<'a>,
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
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::extension::interest_bearing_mint::instruction::update_rate(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.rate_authority.address(),
        &signer_addresses,
        rate,
    )?;
    ctx.invoke_ix(ix)
}
