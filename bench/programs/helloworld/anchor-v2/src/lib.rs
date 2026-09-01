use anchor_lang::prelude::*;

declare_id!("B7ihZyoXZ1fwAY3TugkiFJ6SXkzJwMuQrxrekBaSmn32");

#[program]
pub mod hello_world_v2 {
    use super::*;

    #[inline(always)]
    pub fn init(ctx: &mut Context<Init>) -> Result<()> {
        let counter = &mut ctx.accounts.counter;
        counter.value = 42;
        counter.bump = ctx.bumps.counter;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Init {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [b"counter"], bump)]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

#[account]
pub struct Counter {
    pub value: u64,
    pub bump: u8,
    pub _pad: [u8; 7],
}
