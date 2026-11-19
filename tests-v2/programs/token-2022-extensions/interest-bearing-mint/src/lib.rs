use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("4Yv95TS6s4kME8qgqLpjkekknuXJnksuCnVbc37Pdp6j");

#[program]
pub mod token_2022_ext_interest_bearing_mint {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>, authority: Address) -> Result<()> {
        let accs = token_2022_ext::InterestBearingMintInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::interest_bearing_mint_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&authority),
            125,
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update_rate(ctx: &mut Context<UpdateRate>) -> Result<()> {
        let accs = token_2022_ext::InterestBearingMintUpdateRate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            rate_authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::interest_bearing_mint_update_rate(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            -125,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(authority: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct UpdateRate {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
