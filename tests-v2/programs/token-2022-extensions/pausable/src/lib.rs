use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("EoHXMZePT9ShHp5tUxBhZQW4MRm4P4r2ejx7VXMpP2My");

#[program]
pub mod token_2022_ext_pausable {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>, authority: Address) -> Result<()> {
        let accs = token_2022_ext::PausableInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::pausable_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            &authority,
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn pause(ctx: &mut Context<Toggle>) -> Result<()> {
        let accs = token_2022_ext::PausableToggle {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::pausable_pause(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }

    #[discrim = 2]
    pub fn resume(ctx: &mut Context<Toggle>) -> Result<()> {
        let accs = token_2022_ext::PausableToggle {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::pausable_resume(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
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
pub struct Toggle {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
