#![allow(warnings)]

use anchor_lang::prelude::*;

declare_id!("New1111111111111111111111111111111111111111");

#[program]
pub mod new {
    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
        Ok(())
    }

    pub fn init_another(ctx: Context<InitAnother>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 40)]
    pub expected_account: Account<'info, ExpectedAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitAnother<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(init, payer = authority, space = 72)]
    pub another_account: Account<'info, AnotherAccount>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct ExpectedAccount {
    pub data: Pubkey,
}

#[account]
pub struct AnotherAccount {
    pub a: Pubkey,
    pub b: Pubkey,
}
