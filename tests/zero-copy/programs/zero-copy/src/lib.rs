//! This example demonstrates the use of zero-copy deserialization for accounts.
//! Zero-copy is a deserialization technique that creates data structures by borrowing
//! (not copying!) from the array holding the input, avoiding the expensive memory
//! allocation and processing of traditional deserialization.
//! With zero-copy, we can create accounts larger than the size of the stack or heap,
//! as is demonstrated by the event queue in this example.

use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod zero_copy {
    use super::*;

    pub fn create_foo(ctx: Context<CreateFoo>) -> Result<()> {
        let foo = &mut ctx.accounts.foo.load_init()?;
        foo.authority = *ctx.accounts.authority.key;
        foo.set_second_authority(ctx.accounts.authority.key);
        Ok(())
    }

    pub fn update_foo(ctx: Context<UpdateFoo>, data: u64) -> Result<()> {
        let mut foo = ctx.accounts.foo.load_mut()?;
        foo.data = data;
        Ok(())
    }

    pub fn update_foo_second(ctx: Context<UpdateFooSecond>, second_data: u64) -> Result<()> {
        let mut foo = ctx.accounts.foo.load_mut()?;
        foo.second_data = second_data;
        Ok(())
    }

    pub fn create_bar(ctx: Context<CreateBar>) -> Result<()> {
        let bar = &mut ctx.accounts.bar.load_init()?;
        bar.authority = *ctx.accounts.authority.key;
        Ok(())
    }

    pub fn update_bar(ctx: Context<UpdateBar>, data: u64) -> Result<()> {
        let bar = &mut ctx.accounts.bar.load_mut()?;
        bar.data = data;
        Ok(())
    }

    pub fn create_large_account(_ctx: Context<CreateLargeAccount>) -> Result<()> {
        Ok(())
    }

    pub fn update_large_account(
        ctx: Context<UpdateLargeAccount>,
        idx: u32,
        data: u64,
    ) -> Result<()> {
        let event_q = &mut ctx.accounts.event_q.load_mut()?;
        event_q.events[idx as usize] = Event {
            data,
            from: *ctx.accounts.from.key,
        };
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SetEvent<'info> {
    authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CreateFoo<'info> {
    #[account(zero)]
    foo: AccountLoader<'info, Foo>,
    authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFoo<'info> {
    #[account(mut, has_one = authority)]
    foo: AccountLoader<'info, Foo>,
    authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct UpdateFooSecond<'info> {
    #[account(
        mut,
        constraint = &foo.load()?.get_second_authority() == second_authority.key,
    )]
    foo: AccountLoader<'info, Foo>,
    second_authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CreateBar<'info> {
    #[account(
        init,
        seeds = [authority.key().as_ref(), foo.key().as_ref()],
        bump,
        payer = authority, owner = ID,
        space = Bar::LEN + 8
    )]
    bar: AccountLoader<'info, Bar>,
    #[account(mut)]
    authority: Signer<'info>,
    foo: AccountLoader<'info, Foo>,
    system_program: UncheckedAccount<'info>,
}
#[derive(Accounts)]
pub struct UpdateBar<'info> {
    #[account(
        mut,
        has_one = authority,
        seeds = [authority.key().as_ref(), foo.key().as_ref()],
        bump,
    )]
    pub bar: AccountLoader<'info, Bar>,
    pub authority: Signer<'info>,
    pub foo: AccountLoader<'info, Foo>,
}

#[derive(Accounts)]
pub struct CreateLargeAccount<'info> {
    #[account(zero)]
    event_q: AccountLoader<'info, EventQ>,
}

#[derive(Accounts)]
pub struct UpdateLargeAccount<'info> {
    #[account(mut)]
    event_q: AccountLoader<'info, EventQ>,
    from: Signer<'info>,
}

#[account(zero_copy)]
#[derive(Default)]
pub struct Foo {
    pub authority: Pubkey,
    pub data: u64,
    pub second_data: u64,
    #[accessor(Pubkey)] // The `accessor` api will likely be removed.
    pub second_authority: [u8; 32],
}

#[account(zero_copy)]
pub struct Bar {
    pub authority: Pubkey, // 32
    pub data: u64,         // 8
}

impl Bar {
    pub const LEN: usize = 32 + 8;
}

#[account(zero_copy)]
pub struct EventQ {
    pub events: [Event; 25000],
}

#[zero_copy]
pub struct Event {
    pub from: Pubkey,
    pub data: u64,
}

// A separate type is used for the RPC interface for two main reasons.
//
// 1. AnchorSerialize and AnchorDeserialize must be derived. Anchor requires
//    *all* instructions to implement the AnchorSerialize and AnchorDeserialize
//    traits, so any types in method signatures must as well.
// 2. All types for zero copy deserialization are `#[repr(packed)]`. However,
//    the implementation of AnchorSerialize (i.e. borsh), uses references
//    to the fields it serializes. So if we were to just throw these derives
//    onto the other `Event` struct, we would have references to
//    `#[repr(packed)]` fields, which is unsafe. To avoid the unsafeness, we
//    just use a separate type.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RpcEvent {
    pub from: Pubkey,
    pub data: u64,
}

impl From<RpcEvent> for Event {
    fn from(e: RpcEvent) -> Event {
        Event {
            from: e.from,
            data: e.data,
        }
    }
}
