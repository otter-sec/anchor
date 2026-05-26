use anchor_lang_v2::prelude::*;

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
        space = 8 + core::mem::size_of::<Counter>(),
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
