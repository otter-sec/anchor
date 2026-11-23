use anchor_lang::{prelude::*, Vec};

declare_id!("E2BJE8WXAe7fLnW1ekVGg75udBFfWefNUsvPNcaKwLMm");

const COUNTER_SEED: &[u8] = b"event-cpi-counter";

#[account]
pub struct Counter {
    pub value: u64,
}

#[event]
pub struct EventCpiObserved {
    pub value: u64,
    pub marker: u8,
}

#[event]
pub struct EventCpiLarge {
    pub payload: [u8; 16],
}

#[program]
pub mod event_cpi_test {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.value = 0;
        Ok(())
    }

    #[discrim = 1]
    pub fn emit_once(ctx: &mut Context<EmitOnce>, value: u64) -> Result<()> {
        ctx.accounts.counter.value = value;
        emit_cpi!(EventCpiObserved { value, marker: 7 });
        Ok(())
    }

    #[discrim = 2]
    pub fn emit_with_growing_storage(ctx: &mut Context<EmitOnce>) -> Result<()> {
        let mut storage = Vec::with_capacity(8);
        emit_cpi!(EventCpiLarge { payload: [7; 16] }, &mut storage);
        require!(storage.len() == 32, Error::InvalidInstructionData);
        ctx.accounts.counter.value = 2;
        Ok(())
    }

    #[discrim = 0xe4]
    pub fn event_cpi_shadow_probe(ctx: &mut Context<Probe>) -> Result<()> {
        ctx.accounts.counter.value = 0xe4;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [COUNTER_SEED],
        bump,
    )]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct EmitOnce {
    #[account(mut, seeds = [COUNTER_SEED], bump)]
    pub counter: Account<Counter>,
}

#[derive(Accounts)]
pub struct Probe {
    #[account(mut, seeds = [COUNTER_SEED], bump)]
    pub counter: Account<Counter>,
}
