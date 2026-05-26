use anchor_lang_v2::__alloc::{string::String, vec::Vec};
use anchor_lang_v2::prelude::*;

declare_id!("BF748KR4UhPq7xbhFQYd7yFKmh5UYdqed9GbD6oZvEyu");
declare_program!(return_spoof);

pub const RETURN_SEED: &[u8] = b"return-store";

#[account]
#[repr(C)]
pub struct ReturnStore {
    pub authority: Address,
    pub last_base: u64,
    pub last_result: u64,
    pub calls: u16,
    pub bump: u8,
    pub _padding: [u8; 5],
}

#[derive(Clone, wincode::SchemaRead, wincode::SchemaWrite)]
pub struct ReturnPayload {
    pub amount: u64,
    pub label: String,
    pub samples: Vec<u16>,
    pub maybe_authority: Option<Address>,
}

#[program]
pub mod return_callee {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.authority = *ctx.accounts.authority.address();
        ctx.accounts.data.last_base = 0;
        ctx.accounts.data.last_result = 0;
        ctx.accounts.data.calls = 0;
        ctx.accounts.data.bump = ctx.bumps.data;
        ctx.accounts.data._padding = [0; 5];
        Ok(())
    }

    #[discrim = 1]
    pub fn calculate(ctx: &mut Context<Calculate>, base: u64, bonus: u16) -> Result<u64> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        let result = base
            .saturating_mul(2)
            .saturating_add(bonus as u64)
            .saturating_add(ctx.accounts.data.calls as u64);
        ctx.accounts.data.last_base = base;
        ctx.accounts.data.last_result = result;
        Ok(result)
    }

    #[discrim = 2]
    pub fn describe(ctx: &mut Context<Calculate>, base: u64, bonus: u16) -> Result<ReturnPayload> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        let amount = base
            .saturating_mul(3)
            .saturating_add(bonus as u64)
            .saturating_add(ctx.accounts.data.calls as u64);
        ctx.accounts.data.last_base = base;
        ctx.accounts.data.last_result = amount;
        let mut samples = Vec::new();
        samples.push(bonus);
        samples.push(ctx.accounts.data.calls);
        Ok(ReturnPayload {
            amount,
            label: String::from("return-payload"),
            samples,
            maybe_authority: Some(*ctx.accounts.authority.address()),
        })
    }

    #[discrim = 3]
    pub fn no_return_but_idl_says(ctx: &mut Context<Calculate>, base: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.last_base = base;
        Ok(())
    }

    #[discrim = 4]
    pub fn short_return(ctx: &mut Context<Calculate>, base: u64) -> Result<[u8; 3]> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.last_base = base;
        Ok([1, 2, 3])
    }

    #[discrim = 5]
    pub fn spoofed_return(ctx: &mut Context<SpoofedReturn>, base: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_spoof::cpi::accounts::Spoof {
            data: ctx.accounts.data.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.spoof_program.address(), cpi_accounts);
        let _ = return_spoof::cpi::spoof(cpi_ctx, base)?;
        Ok(())
    }

    #[discrim = 6]
    pub fn malformed_payload(ctx: &mut Context<Calculate>, base: u64) -> Result<u64> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.last_base = base;
        Ok(base)
    }

    #[discrim = 7]
    pub fn describe_empty(
        ctx: &mut Context<Calculate>,
        base: u64,
        bonus: u16,
    ) -> Result<ReturnPayload> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        let amount = base
            .saturating_mul(5)
            .saturating_add(bonus as u64)
            .saturating_add(ctx.accounts.data.calls as u64);
        ctx.accounts.data.last_base = base;
        ctx.accounts.data.last_result = amount;
        Ok(ReturnPayload {
            amount,
            label: String::new(),
            samples: Vec::new(),
            maybe_authority: None,
        })
    }
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<ReturnStore>(),
        seeds = [RETURN_SEED, authority.address().as_ref()],
        bump,
    )]
    pub data: Account<ReturnStore>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Calculate {
    #[account(mut, seeds = [RETURN_SEED, authority.address().as_ref()], bump)]
    pub data: Account<ReturnStore>,
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct SpoofedReturn {
    #[account(mut, seeds = [RETURN_SEED, authority.address().as_ref()], bump)]
    pub data: Account<ReturnStore>,
    pub authority: Signer,
    pub spoof_program: UncheckedAccount,
}
