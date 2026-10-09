//! Ensures the `seeds = …` attribute accepts both
//!   1. A literal slice `[ … ]`, and
//!   2. An arbitrary expression that evaluates to `&[&[u8]]`.
//!
//! The file only needs to **compile**; no runtime logic executes.
//
//! Implementation note on leaks: we leak a few bytes per call in
//! `pda_seeds`.  That is harmless for on‑chain programs because the
//! binary never unloads.  Once the Anchor tests can use nightly we can
//! replace it with a `const fn` + `OnceLock` that avoids the leak.

#![allow(dead_code)]

extern crate anchor_lang_v1 as anchor_lang;

use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const PREFIX: &[u8] = b"prefix";

/// Builds a `'static` seed slice from `key`
fn pda_seeds(key: Pubkey) -> &'static [&'static [u8]] {
    let key_bytes: &'static [u8; 32] = Box::leak(Box::new(key.to_bytes()));
    // leak `[PREFIX, key_bytes]` and coerce to a slice
    Box::leak(Box::new([PREFIX, key_bytes])) as &'static [&[u8]]
}

#[derive(Accounts)]
pub struct LiteralSeeds<'info> {
    #[account(
        // Literal list parsed as `SeedsExpr::List`
        seeds = [PREFIX, user.key().as_ref()],
        bump
    )]
    pda: Account<'info, Dummy>,
    #[account(
        // Literal list with a trailing comma parsed as `SeedsExpr::List`
        seeds = [PREFIX, user.key().as_ref(),],
        bump
    )]
    user: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExprSeeds<'info> {
    #[account(
        // Expression parsed as `SeedsExpr::Expr`
        seeds = pda_seeds(user.key()),
        bump
    )]
    pda: Account<'info, Dummy>,
    user: Signer<'info>,
}

/// Dummy account so the structs derive cleanly
#[account]
pub struct Dummy {}
