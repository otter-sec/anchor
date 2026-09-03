use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("8PeNs8jhrvR4uDtSSyB2iYcyx5FUQBWhrnHJfwdHwXiS");

#[program]
pub mod token_2022_ext_metadata_pointer {
    use super::*;

    #[discrim = 0]
    pub fn initialize(
        ctx: &mut Context<Initialize>,
        authority: Address,
        metadata_address: Address,
    ) -> Result<()> {
        let accs = token_2022_ext::MetadataPointerInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::metadata_pointer_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&authority),
            Some(&metadata_address),
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update(ctx: &mut Context<Update>, metadata_address: Address) -> Result<()> {
        let accs = token_2022_ext::MetadataPointerUpdate {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::metadata_pointer_update(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&metadata_address),
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(authority: Address, metadata_address: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(metadata_address: Address)]
pub struct Update {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}
