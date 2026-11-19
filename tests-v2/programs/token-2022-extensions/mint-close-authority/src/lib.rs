use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("3riR5k4baKpAn75dhjKgQJtZHVRfsKy5211q2zxphgbC");

#[program]
pub mod token_2022_ext_mint_close_authority {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>, close_authority: Address) -> Result<()> {
        let accs = token_2022_ext::MintCloseAuthorityInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::mint_close_authority_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&close_authority),
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(close_authority: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}
