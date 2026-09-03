use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("7aCUXoc5WNTQUVTeT7mJ6hXGAdR6fXumTZVFF3zoV1cV");

#[program]
pub mod token_2022_ext_cpi_guard {
    use super::*;

    #[allow(deprecated)]
    #[discrim = 0]
    pub fn enable(ctx: &mut Context<Toggle>) -> Result<()> {
        let accs = token_2022_ext::CpiGuard {
            account: ctx.accounts.account.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
        };
        token_2022_ext::cpi_guard_enable(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }

    #[allow(deprecated)]
    #[discrim = 1]
    pub fn disable(ctx: &mut Context<Toggle>) -> Result<()> {
        let accs = token_2022_ext::CpiGuard {
            account: ctx.accounts.account.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
        };
        token_2022_ext::cpi_guard_disable(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Toggle {
    #[account(mut)]
    pub account: UncheckedAccount,
    pub owner: Signer,
    pub token_program: UncheckedAccount,
}
