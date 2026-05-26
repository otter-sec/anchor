use anchor_lang_v2::prelude::*;

declare_id!("D9t6cEFPTDWmTZfcikokLbnuuyeJT6oXnpEbyXB45LU2");

pub const OPTIONAL_SEED: &[u8] = b"declared-optional";

#[account]
#[repr(C)]
pub struct OptionalStore {
    pub value: u64,
    pub calls: u16,
    pub saw_marker: u8,
    pub bump: u8,
    pub authority: Address,
    pub marker: Address,
    pub _pad: [u8; 4],
}

#[program]
pub mod optional_callee {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.value = 0;
        ctx.accounts.data.calls = 0;
        ctx.accounts.data.saw_marker = 0;
        ctx.accounts.data.bump = ctx.bumps.data;
        ctx.accounts.data.authority = *ctx.accounts.authority.address();
        ctx.accounts.data.marker = Address::default();
        ctx.accounts.data._pad = [0; 4];
        Ok(())
    }

    #[discrim = 1]
    pub fn record(ctx: &mut Context<Record>, value: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.value = value.saturating_add(ctx.accounts.data.calls as u64);
        if let Some(marker) = &ctx.accounts.maybe_marker {
            ctx.accounts.data.saw_marker = 1;
            ctx.accounts.data.marker = *marker.address();
        } else {
            ctx.accounts.data.saw_marker = 0;
            ctx.accounts.data.marker = Address::default();
        }
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
        space = 8 + core::mem::size_of::<OptionalStore>(),
        seeds = [OPTIONAL_SEED, authority.address().as_ref()],
        bump,
    )]
    pub data: Account<OptionalStore>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Record {
    #[account(mut, seeds = [OPTIONAL_SEED, authority.address().as_ref()], bump)]
    pub data: Account<OptionalStore>,
    pub authority: Signer,
    pub maybe_marker: Option<UncheckedAccount>,
}
