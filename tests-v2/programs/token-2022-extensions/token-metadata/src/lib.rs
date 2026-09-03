use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("7j7C1skvNNAm1GPr6icFPbdsare3eYpQGrtuzXJ6jzy6");

#[program]
pub mod token_2022_ext_token_metadata {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        let accs = token_2022_ext::TokenMetadataInitialize {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
            mint: ctx.accounts.mint.cpi_handle(),
            mint_authority: ctx.accounts.mint_authority.cpi_handle(),
        };
        token_2022_ext::token_metadata_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            "name".into(),
            "SYM".into(),
            "https://example.invalid".into(),
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn update_authority(
        ctx: &mut Context<UpdateAuthority>,
        new_authority: Address,
    ) -> Result<()> {
        let accs = token_2022_ext::TokenMetadataUpdateAuthority {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            current_authority: ctx.accounts.current_authority.cpi_handle(),
        };
        token_2022_ext::token_metadata_update_authority(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&new_authority),
        )?;
        Ok(())
    }

    #[discrim = 2]
    pub fn update_field(ctx: &mut Context<UpdateField>) -> Result<()> {
        let accs = token_2022_ext::TokenMetadataUpdateField {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
        };
        token_2022_ext::token_metadata_update_field(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            token_2022_ext::token_metadata::Field::Key("field".into()),
            "value".into(),
        )?;
        Ok(())
    }

    #[discrim = 3]
    pub fn remove_key(ctx: &mut Context<RemoveKey>) -> Result<()> {
        let accs = token_2022_ext::TokenMetadataRemoveKey {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
        };
        token_2022_ext::token_metadata_remove_key(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            "field".into(),
            true,
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut, unsafe(dup))]
    pub metadata: UncheckedAccount,
    pub update_authority: UncheckedAccount,
    #[account(unsafe(dup))]
    pub mint: UncheckedAccount,
    pub mint_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct UpdateAuthority {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub current_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct UpdateField {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct RemoveKey {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub token_program: UncheckedAccount,
}
