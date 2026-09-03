use anchor_lang::prelude::*;

#[account]
pub struct Oracle {
    pub authority: Address,
    pub price: u64,
}
