use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("DEYmLQZNGBBhzQM8vqfhKerrcybk3PxMXYj62NY8gwZR");

#[program]
pub mod token_2022_ext_immutable_owner {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        let accs = token_2022_ext::ImmutableOwnerInitialize {
            token_account: ctx.accounts.token_account.cpi_handle_mut(),
        };
        token_2022_ext::immutable_owner_initialize(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub token_account: UncheckedAccount,
    pub token_program: UncheckedAccount,
}
