#![allow(warnings)]

use anchor_lang::prelude::*;

declare_id!("oLd1111111111111111111111111111111111111111");

#[program]
pub mod old {
    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
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

#[account]
pub struct ExpectedAccount {
    pub data: Pubkey,
}
