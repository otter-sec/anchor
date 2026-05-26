use anchor_lang_v2::prelude::*;

declare_id!("FfuEBk58icFrsQX6rPQEKS2bQzvCjRrgErasDce6KsD7");

pub const HASH_SEED: &[u8] = b"declared-hash";

#[account]
#[repr(C)]
pub struct HashStore {
    pub value: i64,
    pub authority: Address,
    pub last_delta: i64,
    pub marker: [u8; 4],
    pub flag: u8,
    pub calls: u8,
    pub bump: u8,
    pub _pad: [u8; 1],
}

#[program]
pub mod hash_cpi {
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.value = 1;
        ctx.accounts.data.authority = *ctx.accounts.authority.address();
        ctx.accounts.data.last_delta = 0;
        ctx.accounts.data.marker = [0; 4];
        ctx.accounts.data.flag = 0;
        ctx.accounts.data.calls = 0;
        ctx.accounts.data.bump = ctx.bumps.data;
        ctx.accounts.data._pad = [0; 1];
        Ok(())
    }

    pub fn apply(ctx: &mut Context<Apply>, delta: i64, flag: bool, marker: [u8; 4]) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.value = ctx
            .accounts
            .data
            .value
            .saturating_add(delta)
            .saturating_add(ctx.accounts.data.calls as i64);
        ctx.accounts.data.last_delta = delta;
        ctx.accounts.data.flag = flag as u8;
        ctx.accounts.data.marker = marker;
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
        space = 8 + core::mem::size_of::<HashStore>(),
        seeds = [HASH_SEED, authority.address().as_ref()],
        bump,
    )]
    pub data: Account<HashStore>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Apply {
    #[account(mut, seeds = [HASH_SEED, authority.address().as_ref()], bump)]
    pub data: Account<HashStore>,
    pub authority: Signer,
}
