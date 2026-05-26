use anchor_lang_v2::prelude::*;

declare_id!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

pub const ALT_SEED: &[u8] = b"declared-alt";

#[account]
#[repr(C)]
pub struct AltStore {
    pub value: u64,
    pub authority: Address,
    pub last_delta: u8,
    pub calls: u8,
    pub authority_first_byte: u8,
    pub _pad: [u8; 5],
}

#[program]
pub mod alt_cpi {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.value = 10;
        ctx.accounts.data.authority = *ctx.accounts.authority.address();
        ctx.accounts.data.last_delta = 0;
        ctx.accounts.data.calls = 0;
        ctx.accounts.data.authority_first_byte = 0;
        ctx.accounts.data._pad = [0; 5];
        Ok(())
    }

    #[discrim = 1]
    pub fn bump(ctx: &mut Context<Bump>, delta: u8) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.value = ctx
            .accounts
            .data
            .value
            .saturating_add(delta as u64)
            .saturating_add(ctx.accounts.data.calls as u64);
        ctx.accounts.data.last_delta = delta;
        ctx.accounts.data.authority_first_byte = ctx.accounts.authority.address().as_ref()[0];
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
        space = 8 + core::mem::size_of::<AltStore>(),
        seeds = [ALT_SEED, authority.address().as_ref()],
        bump,
    )]
    pub data: Account<AltStore>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct Bump {
    #[account(mut, seeds = [ALT_SEED, authority.address().as_ref()], bump)]
    pub data: Account<AltStore>,
    pub authority: Signer,
}
