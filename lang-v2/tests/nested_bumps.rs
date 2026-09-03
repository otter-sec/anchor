#![allow(dead_code)]

use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Inner {
    #[account(seeds = [b"vault"], bump)]
    pub vault: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Outer {
    pub authority: UncheckedAccount,
    pub inner: Nested<Inner>,
}

fn nested_bump(ctx: &Context<'_, Outer>) -> u8 {
    ctx.bumps.inner.vault
}

#[test]
fn nested_bumps_are_available_through_context() {
    let _: fn(&Context<'_, Outer>) -> u8 = nested_bump;
}
