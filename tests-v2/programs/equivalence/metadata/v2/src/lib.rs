use {anchor_lang::prelude::*, anchor_spl::metadata};

declare_id!("AnQXhs18cC2Q6xqUhPoov4DiYiypsbfY95Mcuy37ZHe5");

#[program]
pub mod equivalence_metadata_v2 {
    use super::*;

    #[discrim = 0]
    pub fn sign_metadata(ctx: &mut Context<SignMetadataProxy>) -> Result<()> {
        let cpi_accounts = metadata::SignMetadata {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            creator: ctx.accounts.creator.cpi_handle(),
        };
        metadata::sign_metadata(CpiContext::new(
            ctx.accounts.metadata_program.address(),
            cpi_accounts,
        ))
    }

    #[discrim = 1]
    pub fn remove_creator_verification(
        ctx: &mut Context<RemoveCreatorVerificationProxy>,
    ) -> Result<()> {
        let cpi_accounts = metadata::RemoveCreatorVerification {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            creator: ctx.accounts.creator.cpi_handle(),
        };
        metadata::remove_creator_verification(CpiContext::new(
            ctx.accounts.metadata_program.address(),
            cpi_accounts,
        ))
    }

    #[discrim = 2]
    pub fn update_primary_sale_happened_via_token(
        ctx: &mut Context<UpdatePrimarySaleHappenedViaTokenProxy>,
    ) -> Result<()> {
        let cpi_accounts = metadata::UpdatePrimarySaleHappenedViaToken {
            metadata: ctx.accounts.metadata.cpi_handle_mut(),
            owner: ctx.accounts.owner.cpi_handle(),
            token: ctx.accounts.token.cpi_handle(),
        };
        metadata::update_primary_sale_happened_via_token(CpiContext::new(
            ctx.accounts.metadata_program.address(),
            cpi_accounts,
        ))
    }

    #[discrim = 3]
    pub fn set_token_standard(ctx: &mut Context<SetTokenStandardProxy>) -> Result<()> {
        let cpi_accounts = metadata::SetTokenStandard {
            metadata_account: ctx.accounts.metadata.cpi_handle_mut(),
            update_authority: ctx.accounts.update_authority.cpi_handle(),
            mint_account: ctx.accounts.mint.cpi_handle(),
        };
        metadata::set_token_standard(
            CpiContext::new(ctx.accounts.metadata_program.address(), cpi_accounts),
            None,
        )
    }
}

#[derive(Accounts)]
pub struct SignMetadataProxy {
    pub metadata_program: UncheckedAccount,
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub creator: UncheckedAccount,
}

#[derive(Accounts)]
pub struct RemoveCreatorVerificationProxy {
    pub metadata_program: UncheckedAccount,
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub creator: UncheckedAccount,
}

#[derive(Accounts)]
pub struct UpdatePrimarySaleHappenedViaTokenProxy {
    pub metadata_program: UncheckedAccount,
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub owner: UncheckedAccount,
    pub token: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SetTokenStandardProxy {
    pub metadata_program: UncheckedAccount,
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: UncheckedAccount,
    pub mint: UncheckedAccount,
}
