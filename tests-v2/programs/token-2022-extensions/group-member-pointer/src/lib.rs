use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("FCvBGDgHxLFU3rgL8jqp4aGJpjJzyyotDKQdrjLhUner");

#[program]
pub mod token_2022_ext_group_member_pointer {
    use super::*;

    #[discrim = 0]
    pub fn initialize(
        ctx: &mut Context<Initialize>,
        authority: Address,
        member_address: Address,
    ) -> Result<()> {
        let accs = token_2022_ext::GroupMemberPointerInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::group_member_pointer_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&authority),
            Some(&member_address),
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update(ctx: &mut Context<Update>, member_address: Address) -> Result<()> {
        let accs = token_2022_ext::GroupMemberPointerUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::group_member_pointer_update(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&member_address),
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(authority: Address, member_address: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(member_address: Address)]
pub struct Update {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
