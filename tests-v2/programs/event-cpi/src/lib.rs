use anchor_lang::{prelude::*, FixedVec};

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

#[event]
pub struct EventCpiPreludeAddress {
    pub authority: Address,
    pub value: u64,
}

/// A local `Address` that shadows the prelude one with a 64-byte encoding.
/// `#[event]` reads the bare name as 32 bytes, so the bound is too small.
pub mod shadowed {
    use anchor_lang::prelude::{event, AnchorDeserialize, AnchorSerialize, IdlType};

    #[derive(AnchorSerialize, AnchorDeserialize, IdlType)]
    pub struct Address(pub [u8; 64]);

    #[event]
    pub struct EventCpiShadowedAddress {
        pub authority: Address,
    }
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
    pub fn emit_overflow_is_atomic(ctx: &mut Context<EmitOnce>) -> Result<()> {
        let mut storage = FixedVec::<u8, 8>::new();
        let result = (|| -> Result<()> {
            emit_cpi!(EventCpiLarge { payload: [7; 16] }, &mut storage);
            Ok(())
        })();
        require!(
            matches!(result, Err(Error::InvalidInstructionData)),
            Error::InvalidInstructionData
        );
        require!(storage.is_empty(), Error::InvalidInstructionData);
        ctx.accounts.counter.value = 2;
        Ok(())
    }

    #[discrim = 3]
    pub fn emit_prelude_address(ctx: &mut Context<EmitOnce>, authority: Address) -> Result<()> {
        emit!(EventCpiPreludeAddress {
            authority,
            value: 3
        });
        emit_cpi!(EventCpiPreludeAddress {
            authority,
            value: 3
        });
        ctx.accounts.counter.value = 3;
        Ok(())
    }

    #[discrim = 4]
    pub fn emit_shadowed_address_overflows(ctx: &mut Context<EmitOnce>) -> Result<()> {
        use shadowed::{Address, EventCpiShadowedAddress};
        let logged = (|| -> Result<()> {
            emit!(EventCpiShadowedAddress {
                authority: Address([7; 64])
            });
            Ok(())
        })();
        require!(
            matches!(logged, Err(Error::InvalidInstructionData)),
            Error::InvalidInstructionData
        );
        let invoked = (|| -> Result<()> {
            emit_cpi!(EventCpiShadowedAddress {
                authority: Address([7; 64])
            });
            Ok(())
        })();
        require!(
            matches!(invoked, Err(Error::InvalidInstructionData)),
            Error::InvalidInstructionData
        );
        ctx.accounts.counter.value = 4;
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
