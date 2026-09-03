//! Test program for the duplicate-mutable-account safety check.
//!
//! Exercises each combination the derive must reject at `try_accounts`
//! time, plus the `unsafe(dup)` escape hatch. The handlers for unsafe
//! variants are written to never hold two `&mut Data` live at once, so
//! invoking them with aliased inputs does not produce UB.

use anchor_lang::prelude::*;

declare_id!("2TxMd2YAMi9Sk4xxiJBNkYQNuxK9FwvwwiujuEbKoanz");

#[program]
pub mod dup_mut {
    use super::*;

    pub fn initialize(_ctx: &mut Context<Initialize>, seed: u8) -> Result<()> {
        let _ = seed;
        Ok(())
    }

    pub fn touch_two_mut(ctx: &mut Context<TouchTwoMut>, value: u64) -> Result<()> {
        ctx.accounts.data_a.value = value;
        ctx.accounts.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_three_mut(ctx: &mut Context<TouchThreeMut>, value: u64) -> Result<()> {
        ctx.accounts.data_a.value = value;
        ctx.accounts.data_b.value = value.wrapping_add(1);
        ctx.accounts.data_c.value = value.wrapping_add(2);
        Ok(())
    }

    pub fn touch_mut_and_readonly(
        ctx: &mut Context<TouchMutAndReadonly>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.data_a.value = value;
        Ok(())
    }

    pub fn touch_two_mut_asym_unsafe(
        ctx: &mut Context<TouchTwoMutAsymUnsafe>,
        value: u64,
    ) -> Result<()> {
        // Reachable only with distinct pubkeys: data_a has no `unsafe(dup)`,
        // so an aliased call still trips the generated check on position 0.
        ctx.accounts.data_a.value = value;
        ctx.accounts.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_two_mut_unsafe(ctx: &mut Context<TouchTwoMutUnsafe>, value: u64) -> Result<()> {
        // SAFETY: When invoked with data_a == data_b, both fields alias the
        // same account data. We only ever materialize ONE `&mut Data` (via
        // `data_a`). `data_b` is never deref'd, so no two `&mut` to the same
        // bytes exist simultaneously and no UB is possible.
        ctx.accounts.data_a.value = value;
        let _ = &ctx.accounts.data_b;
        Ok(())
    }

    // -- Nested<Inner> variants ----------------------------------------------
    //
    // Each of the direct-field scenarios above is mirrored through a
    // `Nested<Inner>` wrapper so the bitvec `base_offset` threading is
    // exercised: the derive's duplicate-mut constraint check uses
    // `__base_offset + offset_expr`, so a bug that dropped the offset would
    // surface as either false positives (distinct accounts rejected) or
    // false negatives (aliased accounts accepted) inside the inner struct.

    pub fn touch_nested_two_mut(ctx: &mut Context<TouchNestedTwoMut>, value: u64) -> Result<()> {
        ctx.accounts.pair.data_a.value = value;
        ctx.accounts.pair.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_nested_three_mut(
        ctx: &mut Context<TouchNestedThreeMut>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.trio.data_a.value = value;
        ctx.accounts.trio.data_b.value = value.wrapping_add(1);
        ctx.accounts.trio.data_c.value = value.wrapping_add(2);
        Ok(())
    }

    pub fn touch_nested_mut_readonly(
        ctx: &mut Context<TouchNestedMutReadonly>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.pair.data_a.value = value;
        Ok(())
    }

    pub fn touch_nested_asym_unsafe(
        ctx: &mut Context<TouchNestedAsymUnsafe>,
        value: u64,
    ) -> Result<()> {
        // Reachable only with distinct pubkeys — see direct-field sibling.
        ctx.accounts.pair.data_a.value = value;
        ctx.accounts.pair.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_nested_unsafe(ctx: &mut Context<TouchNestedUnsafe>, value: u64) -> Result<()> {
        // SAFETY: same argument as touch_two_mut_unsafe — only data_a is
        // deref'd, so no two live `&mut Data` to the same bytes coexist.
        ctx.accounts.pair.data_a.value = value;
        let _ = &ctx.accounts.pair.data_b;
        Ok(())
    }

    // Cross-boundary: outer mut account sitting next to Nested<Pair>. Lets
    // tests alias the outer field against either of the inner fields and
    // confirm the check fires regardless of which side of the boundary the
    // duplicate lives on.
    pub fn touch_outer_mut_plus_nested(
        ctx: &mut Context<TouchOuterMutPlusNested>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.outer.value = value;
        ctx.accounts.pair.data_a.value = value.wrapping_add(1);
        ctx.accounts.pair.data_b.value = value.wrapping_add(2);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(seed: u8)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"d", &seed.to_le_bytes()],
        bump,
    )]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TouchTwoMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchThreeMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
    #[account(mut)]
    pub data_c: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchMutAndReadonly {
    #[account(mut)]
    pub data_a: Account<Data>,
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchTwoMutAsymUnsafe {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut, unsafe(dup))]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchTwoMutUnsafe {
    #[account(mut, unsafe(dup))]
    pub data_a: Account<Data>,
    #[account(mut, unsafe(dup))]
    pub data_b: Account<Data>,
}

// --- Inner `Accounts` structs (embedded via `Nested<_>`) -------------------
//
// These mirror the direct-field variants one-for-one. They are plain
// `#[derive(Accounts)]` structs, so the derive emits a `TryAccounts` impl
// that the outer struct's generated `try_accounts` delegates to via
// `Inner::try_accounts(..., __base_offset + offset_expr, ...)`.

#[derive(Accounts)]
pub struct InnerTwoMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct InnerThreeMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
    #[account(mut)]
    pub data_c: Account<Data>,
}

#[derive(Accounts)]
pub struct InnerMutReadonly {
    #[account(mut)]
    pub data_a: Account<Data>,
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct InnerAsymUnsafe {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut, unsafe(dup))]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct InnerUnsafe {
    #[account(mut, unsafe(dup))]
    pub data_a: Account<Data>,
    #[account(mut, unsafe(dup))]
    pub data_b: Account<Data>,
}

// --- Outer instructions that wrap each Inner via Nested<_> -----------------

#[derive(Accounts)]
pub struct TouchNestedTwoMut {
    pub pair: Nested<InnerTwoMut>,
}

#[derive(Accounts)]
pub struct TouchNestedThreeMut {
    pub trio: Nested<InnerThreeMut>,
}

#[derive(Accounts)]
pub struct TouchNestedMutReadonly {
    pub pair: Nested<InnerMutReadonly>,
}

#[derive(Accounts)]
pub struct TouchNestedAsymUnsafe {
    pub pair: Nested<InnerAsymUnsafe>,
}

#[derive(Accounts)]
pub struct TouchNestedUnsafe {
    pub pair: Nested<InnerUnsafe>,
}

#[derive(Accounts)]
pub struct TouchOuterMutPlusNested {
    #[account(mut)]
    pub outer: Account<Data>,
    pub pair: Nested<InnerTwoMut>,
}

#[account]
pub struct Data {
    pub value: u64,
}
