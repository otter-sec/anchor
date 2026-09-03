use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("BrUvjfkhwnq2oL4uvHCEZ3LDxaXsqbjUCvGUq4LtDHAb");

#[program]
pub mod token_2022_ext_token_group {
    use super::*;

    #[discrim = 0]
    pub fn initialize_group(ctx: &mut Context<InitializeGroup>) -> Result<()> {
        let accs = token_2022_ext::TokenGroupInitialize {
            group: ctx.accounts.group.cpi_handle_mut(),
            mint: ctx.accounts.mint.cpi_handle(),
            mint_authority: ctx.accounts.mint_authority.cpi_handle(),
        };
        token_2022_ext::token_group_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(ctx.accounts.mint_authority.address()),
            10,
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn initialize_member(ctx: &mut Context<InitializeMember>) -> Result<()> {
        let accs = token_2022_ext::TokenMemberInitialize {
            member: ctx.accounts.member.cpi_handle_mut(),
            member_mint: ctx.accounts.member_mint.cpi_handle(),
            member_mint_authority: ctx.accounts.member_mint_authority.cpi_handle(),
            group: ctx.accounts.group.cpi_handle_mut(),
            group_update_authority: ctx.accounts.group_update_authority.cpi_handle(),
        };
        token_2022_ext::token_member_initialize(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeGroup {
    #[account(mut, unsafe(dup))]
    pub group: UncheckedAccount,
    #[account(unsafe(dup))]
    pub mint: UncheckedAccount,
    pub mint_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct InitializeMember {
    #[account(mut, unsafe(dup))]
    pub member: UncheckedAccount,
    #[account(unsafe(dup))]
    pub member_mint: UncheckedAccount,
    pub member_mint_authority: Signer,
    #[account(mut)]
    pub group: UncheckedAccount,
    pub group_update_authority: Signer,
    pub token_program: UncheckedAccount,
}
