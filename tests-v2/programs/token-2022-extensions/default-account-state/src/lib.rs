use {
    anchor_lang::prelude::*,
    anchor_spl::{token_2022::spl_token_2022, token_2022_extensions as token_2022_ext},
};

declare_id!("Fetkn8caf7wN24u751NWUYhtXXGuCPrqTLyDtqU25EY8");

#[program]
pub mod token_2022_ext_default_account_state {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        let accs = token_2022_ext::DefaultAccountStateInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::default_account_state_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            &spl_token_2022::state::AccountState::Frozen,
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update(ctx: &mut Context<Update>) -> Result<()> {
        let accs = token_2022_ext::DefaultAccountStateUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            freeze_authority: ctx.accounts.freeze_authority.cpi_handle(),
        };
        token_2022_ext::default_account_state_update(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            &spl_token_2022::state::AccountState::Initialized,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Update {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub freeze_authority: Signer,
    pub token_program: UncheckedAccount,
}
