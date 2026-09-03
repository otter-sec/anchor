use {
    anchor_lang::prelude::*,
    anchor_spl::{
        mint::{self, Mint},
        token::Token,
    },
};

declare_id!("CrVfxA2g7VqBvkYQG4eCyz8YdVCrbsnY6SQWL6gNw7h5");

#[program]
pub mod constraint_values {
    use super::*;

    #[discrim = 0]
    pub fn init_mint(_ctx: &mut Context<InitMint>) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn check_optional_authority(_ctx: &mut Context<CheckOptionalAuthority>) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn init_mint_update_only_decimals(
        _ctx: &mut Context<InitMintUpdateOnlyDecimals>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitMint {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CheckOptionalAuthority {
    pub authority: Option<Signer>,
    #[account(mut, mint::authority = authority)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct InitMintUpdateOnlyDecimals {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
    #[account(
        init,
        payer = payer,
        update(mint::decimals = 9),
        mint::authority = authority,
    )]
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}
