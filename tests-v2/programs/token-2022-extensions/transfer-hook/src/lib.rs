use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("Bs5CGVSvcNqrTyzZig9fVHcDZhCmvFeserfVFK7BiSjR");

#[program]
pub mod token_2022_ext_transfer_hook {
    use super::*;

    #[discrim = 0]
    pub fn initialize(
        ctx: &mut Context<Initialize>,
        authority: Address,
        program_id: Address,
    ) -> Result<()> {
        let accs = token_2022_ext::TransferHookInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::transfer_hook_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&authority),
            Some(&program_id),
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update(ctx: &mut Context<Update>, program_id: Address) -> Result<()> {
        let accs = token_2022_ext::TransferHookUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::transfer_hook_update(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&program_id),
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(authority: Address, program_id: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(program_id: Address)]
pub struct Update {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
