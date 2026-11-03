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

    pub fn legacy_local_name(ctx: Context<LegacyLocalName>, a: u64, b: u32) -> Result<()> {
        msg!("legacy args: {}, {}", a, b);
        ctx.accounts.child.dummy_a.data = u64::from(b);
        Ok(())
    }

    pub fn explicit_seed(ctx: Context<ExplicitSeed>, prefix: String, value: u32) -> Result<()> {
        msg!("selected args: {}, {}", prefix, value);
        ctx.accounts.child.dummy_a.data = u64::from(value);
        Ok(())
    }

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
    #[instruction(start = value_a)]
    foo: FooWithArgs<'info>,
    #[instruction(start = value_b)]
    bar: BarWithArgs<'info>,
}

#[derive(Accounts)]
#[instruction(value_a: u64)]
pub struct FooWithArgs<'info> {
    #[account(mut, constraint = value_a != 0 @ CompositeError::ZeroValueA)]
    pub dummy_a: Account<'info, DummyA>,
}

#[derive(Accounts)]
#[instruction(value_b: u64)]
pub struct BarWithArgs<'info> {
    #[account(mut, constraint = value_b != 0 @ CompositeError::ZeroValueB)]
    pub dummy_b: Account<'info, DummyB>,
}

#[error_code]
pub enum CompositeError {
    #[msg("local child argument must be nine")]
    WrongLocalValue,
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

#[derive(Accounts)]
#[instruction(a: u64)]
pub struct LegacyLocalName<'info> {
    pub child: LegacyLocalChild<'info>,
}

#[derive(Accounts)]
#[instruction(a: u32)]
pub struct LegacyLocalChild<'info> {
    #[account(mut, constraint = a == 9 @ CompositeError::WrongLocalValue)]
    pub dummy_a: Account<'info, DummyA>,
}

#[derive(Accounts)]
#[instruction(prefix: String, value: u32)]
pub struct ExplicitSeed<'info> {
    #[instruction(start = value)]
    pub child: SeedChild<'info>,
}

#[derive(Accounts)]
#[instruction(local_value: u32)]
pub struct SeedChild<'info> {
    /// CHECK: address is checked by seeds
    #[account(seeds = [b"selected", local_value.to_le_bytes().as_ref()], bump)]
    pub target: UncheckedAccount<'info>,
    #[account(mut)]
    pub dummy_a: Account<'info, DummyA>,
}
