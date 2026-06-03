use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Data {
    pub someone: Pubkey,
}

#[account]
#[derive(InitSpace)]
pub struct Another {}
