use crate::state::Counter;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(
        mut,
        seeds = [b"counter", payer.key().as_ref()],
        bump = counter.bump,
    )]
    pub counter: Account<'info, Counter>,
    pub payer: Signer<'info>,
}

pub fn handler(ctx: Context<Update>, data: u64) -> Result<()> {
    ctx.accounts.counter.count = data;
    Ok(())
}
