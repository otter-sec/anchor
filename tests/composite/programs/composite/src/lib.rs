//! This example demonstrates the ability to compose together multiple
//! structs deriving `Accounts`. See `CompositeUpdate`, below.

use anchor_lang::prelude::*;

declare_id!("EHthziFziNoac9LBGxEaVN47Y3uUiRoXvqAiR6oes4iU");

#[program]
mod composite {
    use super::*;
    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn composite_update(
        ctx: Context<CompositeUpdate>,
        dummy_a: u64,
        dummy_b: u64,
    ) -> Result<()> {
        let a = &mut ctx.accounts.foo.dummy_a;
        let b = &mut ctx.accounts.bar.dummy_b;

        a.data = dummy_a;
        b.data = dummy_b;

        Ok(())
    }

    /// Like `composite_update`, but every `Accounts` struct in the tree
    /// declares the instruction args via `#[instruction(..)]`. Regression
    /// for #2942: nested structs must see the args from the start of the
    /// instruction data, not from where the parent stopped reading.
    pub fn composite_update_with_args(
        ctx: Context<CompositeUpdateWithArgs>,
        value_a: u64,
        value_b: u64,
    ) -> Result<()> {
        let a = &mut ctx.accounts.foo.dummy_a;
        let b = &mut ctx.accounts.bar.dummy_b;

        a.data = value_a;
        b.data = value_b;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(zero)]
    pub dummy_a: Account<'info, DummyA>,
    #[account(zero)]
    pub dummy_b: Account<'info, DummyB>,
}

#[derive(Accounts)]
pub struct CompositeUpdate<'info> {
    foo: Foo<'info>,
    bar: Bar<'info>,
}

#[derive(Accounts)]
pub struct Foo<'info> {
    #[account(mut)]
    pub dummy_a: Account<'info, DummyA>,
}

#[derive(Accounts)]
pub struct Bar<'info> {
    #[account(mut)]
    pub dummy_b: Account<'info, DummyB>,
}

#[derive(Accounts)]
#[instruction(value_a: u64, value_b: u64)]
pub struct CompositeUpdateWithArgs<'info> {
    foo: FooWithArgs<'info>,
    bar: BarWithArgs<'info>,
}

/// Declares only a prefix of the parent's args.
#[derive(Accounts)]
#[instruction(value_a: u64)]
pub struct FooWithArgs<'info> {
    #[account(mut, constraint = value_a != 0 @ CompositeError::ZeroValueA)]
    pub dummy_a: Account<'info, DummyA>,
}

/// Declares the same args as the parent.
#[derive(Accounts)]
#[instruction(value_a: u64, value_b: u64)]
pub struct BarWithArgs<'info> {
    #[account(mut, constraint = value_b != 0 @ CompositeError::ZeroValueB)]
    pub dummy_b: Account<'info, DummyB>,
}

#[error_code]
pub enum CompositeError {
    #[msg("value_a must not be zero")]
    ZeroValueA,
    #[msg("value_b must not be zero")]
    ZeroValueB,
}

#[account]
pub struct DummyA {
    pub data: u64,
}

#[account]
pub struct DummyB {
    pub data: u64,
}
