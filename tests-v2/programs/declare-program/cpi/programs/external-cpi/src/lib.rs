use anchor_lang_v2::prelude::*;

declare_id!("BF748KR4UhPq7xbhFQYd7yFKmh5UYdqed9GbD6oZvEyu");

pub const STORE_SEED: &[u8] = b"declared-store";

#[account]
#[repr(C)]
pub struct Store {
    pub value: u64,
    pub last_count: u16,
    pub calls: u16,
    pub bump: u8,
    pub authority: Address,
    pub last_owner: Address,
    pub last_tag: [u8; 3],
}

#[derive(Clone, Copy, wincode::SchemaRead, wincode::SchemaWrite)]
pub struct MyArgs {
    pub amount: u64,
    pub tag: [u8; 3],
    pub owner: Address,
}

#[program]
pub mod external_cpi {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.value = 0;
        ctx.accounts.data.authority = *ctx.accounts.authority.address();
        ctx.accounts.data.last_tag = *b"ini";
        ctx.accounts.data.last_owner = Address::default();
        ctx.accounts.data.last_count = 0;
        ctx.accounts.data.calls = 0;
        ctx.accounts.data.bump = ctx.bumps.data;
        Ok(())
    }

    #[discrim = 1]
    pub fn set_value(ctx: &mut Context<SetValue>, value: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.value = value.saturating_add(ctx.accounts.data.calls as u64);
        ctx.accounts.data.last_tag = *b"set";
        ctx.accounts.data.last_owner = *ctx.accounts.authority.address();
        ctx.accounts.data.last_count = 0;
        Ok(())
    }

    #[discrim = 2]
    pub fn composite(ctx: &mut Context<Composite>, count: u16) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.inner.data.authority,
            *ctx.accounts.inner.authority.address()
        );
        ctx.accounts.inner.data.calls = ctx.accounts.inner.data.calls.saturating_add(1);
        ctx.accounts.inner.data.value = ctx
            .accounts
            .inner
            .data
            .value
            .saturating_add(count as u64)
            .saturating_add(ctx.accounts.inner.data.calls as u64);
        ctx.accounts.inner.data.last_tag = *b"cmp";
        ctx.accounts.inner.data.last_owner = *ctx.accounts.inner.authority.address();
        ctx.accounts.inner.data.last_count = count;
        Ok(())
    }

    #[discrim = 3]
    pub fn defined_args(ctx: &mut Context<SetValue>, args: MyArgs) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address()
        );
        require_keys_eq!(args.owner, *ctx.accounts.authority.address());
        ctx.accounts.data.calls = ctx.accounts.data.calls.saturating_add(1);
        ctx.accounts.data.value = args.amount.saturating_add(ctx.accounts.data.calls as u64);
        ctx.accounts.data.last_tag = args.tag;
        ctx.accounts.data.last_owner = args.owner;
        ctx.accounts.data.last_count = 0;
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
        space = 8 + core::mem::size_of::<Store>(),
        seeds = [STORE_SEED, authority.address().as_ref()],
        bump,
    )]
    pub data: Account<Store>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct SetValue {
    #[account(mut, seeds = [STORE_SEED, authority.address().as_ref()], bump)]
    pub data: Account<Store>,
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct Inner {
    #[account(mut, seeds = [STORE_SEED, authority.address().as_ref()], bump)]
    pub data: Account<Store>,
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct Composite {
    pub inner: Nested<Inner>,
    #[account(mut)]
    pub payer: Signer,
}
