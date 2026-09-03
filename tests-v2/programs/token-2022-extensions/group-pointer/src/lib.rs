use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("3aVa6BL8bgD4My8vgUABgjGSBTYpHQ6Ft41wC3H5EQ5f");

#[program]
pub mod token_2022_ext_group_pointer {
    use super::*;

    #[discrim = 0]
    pub fn initialize(
        ctx: &mut Context<Initialize>,
        authority: Address,
        group_address: Address,
    ) -> Result<()> {
        let accs = token_2022_ext::GroupPointerInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::group_pointer_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&authority),
            Some(&group_address),
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update(ctx: &mut Context<Update>, group_address: Address) -> Result<()> {
        let accs = token_2022_ext::GroupPointerUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::group_pointer_update(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&group_address),
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(authority: Address, group_address: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(group_address: Address)]
pub struct Update {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
