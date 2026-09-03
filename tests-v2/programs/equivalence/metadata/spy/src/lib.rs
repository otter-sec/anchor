use anchor_lang::prelude::*;

declare_id!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

#[program]
pub mod equivalence_metadata_spy {
    use super::*;

    #[discrim = 7]
    pub fn sign_metadata(_ctx: &mut Context<CreatorMutation>) -> Result<()> {
        Ok(())
    }

    #[discrim = 28]
    pub fn remove_creator_verification(_ctx: &mut Context<CreatorMutation>) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn update_primary_sale_happened_via_token(
        _ctx: &mut Context<PrimarySaleUpdate>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 35]
    pub fn set_token_standard(_ctx: &mut Context<SetTokenStandard>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreatorMutation {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub creator: Signer,
}

#[derive(Accounts)]
pub struct PrimarySaleUpdate {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub owner: Signer,
    pub token: UncheckedAccount,
}

#[derive(Accounts)]
pub struct SetTokenStandard {
    #[account(mut)]
    pub metadata: UncheckedAccount,
    pub update_authority: Signer,
    pub mint: UncheckedAccount,
}
