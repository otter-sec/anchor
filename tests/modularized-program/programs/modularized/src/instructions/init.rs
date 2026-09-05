use crate::state::Counter;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(data: u64)]
pub struct Init<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + 8 + 1,
        seeds = [b"counter", payer.key().as_ref()],
        bump
    )]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Init>, data: u64) -> Result<()> {
    let counter = &mut ctx.accounts.counter;
    counter.count = data;
    counter.bump = ctx.bumps.counter;
    Ok(())
}
