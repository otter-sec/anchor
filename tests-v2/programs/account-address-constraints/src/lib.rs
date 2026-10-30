//! Examples for account-like field references in SPL constraints.
//!
//! These examples use typed and optional account wrappers as
//! `mint::authority` / `token::authority` sibling fields.

use {
    anchor_lang::prelude::*,
    anchor_spl::{
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
    pub fn check_mint_with_optional_authority(
        _ctx: &mut Context<CheckMintWithOptionalAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn check_token_with_optional_authority(
        _ctx: &mut Context<CheckTokenWithOptionalAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn check_ata_with_optional_refs(
        _ctx: &mut Context<CheckAtaWithOptionalRefs>,
    ) -> Result<()> {
        Ok(())
    }
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
