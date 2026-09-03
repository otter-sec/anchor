//! Examples for account-like field references in SPL constraints.
//!
//! These examples use boxed and optional account wrappers as
//! `mint::authority` / `token::authority` sibling fields, showing that SPL
//! constraints can refer to any account wrapper with an account address.

use {
    anchor_lang::prelude::*,
    anchor_spl::{
        associated_token::AssociatedToken,
        mint::{self, Mint},
        token::{self, Token, TokenAccount},
    },
};

declare_id!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

#[account]
pub struct AuthorityData {
    pub value: u64,
}

#[program]
pub mod account_address_constraints {
    use super::*;

    #[discrim = 0]
    pub fn init_mint_with_box_authority(
        _ctx: &mut Context<InitMintWithBoxAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn init_token_with_box_authority(
        _ctx: &mut Context<InitTokenWithBoxAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn check_mint_with_box_authority(
        _ctx: &mut Context<CheckMintWithBoxAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 3]
    pub fn check_token_with_box_authority(
        _ctx: &mut Context<CheckTokenWithBoxAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn check_mint_with_optional_authority(
        _ctx: &mut Context<CheckMintWithOptionalAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn check_token_with_optional_authority(
        _ctx: &mut Context<CheckTokenWithOptionalAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 6]
    pub fn init_ata_with_box_refs(_ctx: &mut Context<InitAtaWithBoxRefs>) -> Result<()> {
        Ok(())
    }

    #[discrim = 7]
    pub fn check_ata_with_box_refs(_ctx: &mut Context<CheckAtaWithBoxRefs>) -> Result<()> {
        Ok(())
    }

    #[discrim = 8]
    pub fn check_ata_with_optional_refs(
        _ctx: &mut Context<CheckAtaWithOptionalRefs>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitMintWithBoxAuthority {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Box<Account<AuthorityData>>,
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
pub struct InitTokenWithBoxAuthority {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    pub authority: Box<Account<AuthorityData>>,
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CheckMintWithBoxAuthority {
    pub authority: Box<Account<AuthorityData>>,
    #[account(mut, mint::authority = authority)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct CheckTokenWithBoxAuthority {
    pub authority: Box<Account<AuthorityData>>,
    #[account(mut, token::authority = authority)]
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckMintWithOptionalAuthority {
    pub authority: Option<Account<AuthorityData>>,
    #[account(mut, mint::authority = authority)]
    pub mint: Account<Mint>,
}

#[derive(Accounts)]
pub struct CheckTokenWithOptionalAuthority {
    pub authority: Option<Account<AuthorityData>>,
    #[account(mut, token::authority = authority)]
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct InitAtaWithBoxRefs {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Box<Account<Mint>>,
    pub authority: Box<Account<AuthorityData>>,
    pub token_program: Box<Program<Token>>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: Account<TokenAccount>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CheckAtaWithBoxRefs {
    pub mint: Box<Account<Mint>>,
    pub authority: Box<Account<AuthorityData>>,
    pub token_program: Box<Program<Token>>,
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: Account<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckAtaWithOptionalRefs {
    pub mint: Option<Account<Mint>>,
    pub authority: Option<Account<AuthorityData>>,
    pub token_program: Option<Program<Token>>,
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: Account<TokenAccount>,
}
