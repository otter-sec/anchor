use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("4wehnPGjiVvNZNSaT6SUfmU9S5AT6LvwZ9g87DDqyhq1");

#[program]
pub mod token_2022_ext_permissioned_burn {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>, authority: Address) -> Result<()> {
        let accs = token_2022_ext::PermissionedBurnInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::permissioned_burn_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            &authority,
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn burn(ctx: &mut Context<Burn>, amount: u64) -> Result<()> {
        token_2022_ext::permissioned_burn(burn_ctx(ctx), amount)?;
        Ok(())
    }

    #[discrim = 2]
    pub fn burn_checked(ctx: &mut Context<Burn>, amount: u64, decimals: u8) -> Result<()> {
        token_2022_ext::permissioned_burn_checked(burn_ctx(ctx), amount, decimals)?;
        Ok(())
    }
}

fn burn_ctx<'a>(
    ctx: &'a mut Context<Burn>,
) -> CpiContext<'a, token_2022_ext::PermissionedBurn<'a>> {
    let accs = token_2022_ext::PermissionedBurn {
        from: ctx.accounts.from.cpi_handle_mut(),
        mint: ctx.accounts.mint.cpi_handle_mut(),
        permissioned_burn_authority: ctx.accounts.permissioned_burn_authority.cpi_handle(),
        authority: ctx.accounts.authority.cpi_handle(),
    };
    CpiContext::new(ctx.accounts.token_program.address(), accs)
}

#[derive(Accounts)]
#[instruction(authority: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Burn {
    #[account(mut)]
    pub from: UncheckedAccount,
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub permissioned_burn_authority: Signer,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
