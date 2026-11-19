use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("6wc58Q2xzU5Lw21XrsJXy31LJbcoTcGbEa3knKj7enwM");

#[program]
pub mod token_2022_ext_memo_transfer {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Toggle>) -> Result<()> {
        let accs = token_2022_ext::MemoTransfer {
            account: ctx.accounts.account.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
        };
        token_2022_ext::memo_transfer_initialize(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }

    #[discrim = 1]
    pub fn disable(ctx: &mut Context<Toggle>) -> Result<()> {
        let accs = token_2022_ext::MemoTransfer {
            account: ctx.accounts.account.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
        };
        token_2022_ext::memo_transfer_disable(CpiContext::new(
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
