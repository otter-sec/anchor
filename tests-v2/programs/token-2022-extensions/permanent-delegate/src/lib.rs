use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("7Xp89PEgC8vJzUMCRZ8itHmfurazLM7SSNJ3R8hyaG6t");

#[program]
pub mod token_2022_ext_permanent_delegate {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>, delegate: Address) -> Result<()> {
        let accs = token_2022_ext::PermanentDelegateInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::permanent_delegate_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            &delegate,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(delegate: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}
