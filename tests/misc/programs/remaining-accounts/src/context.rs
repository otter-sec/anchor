use crate::account::*;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;

#[derive(Accounts)]
pub struct TestInit<'info> {
    #[account(init, payer = payer, space = Data::INIT_SPACE + 8)]
    pub data: Account<'info, Data>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestInitAnother<'info> {
    #[account(init, payer = payer, space = Data::INIT_SPACE + 8)]
    pub another: Account<'info, Another>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestRemainingAccounts<'info> {
    pub token_program: Program<'info, Token>,
}
