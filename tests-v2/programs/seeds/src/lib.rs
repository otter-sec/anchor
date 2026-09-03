//! Test program for PDA seeds in every supported form.
//!
//! Covers:
//!   - `seeds = [b"literal"]` (array-bracket, const-PDA eligible)
//!   - `seeds = [b"tag", user.address().as_ref()]` (array-bracket, mixed)
//!   - `seeds = my_fn()` (expression, function call)
//!   - `seeds = CONST_SEEDS` (expression, const item)
//!   - explicit `bump = data.bump` with each seeds form
//!   - bare `bump` (runtime find) with each seeds form

use anchor_lang::prelude::*;

declare_id!("Hyc9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

// -- Helpers for expression-form seeds ---------------------------------------

fn tag_seeds() -> [&'static [u8]; 1] {
    [b"data"]
}

const CONST_TAG_SEEDS: [&[u8]; 1] = [b"data"];

// -- Account types -----------------------------------------------------------

#[account]
pub struct Data {
    pub value: u64,
    pub bump: u8,
    pub _pad: [u8; 7],
}

#[account]
pub struct Manager {
    pub next_oracle_id: u64,
}

#[account]
pub struct Oracle {
    pub bump: u8,
}

// -- Instructions ------------------------------------------------------------

#[program]
pub mod seeds {
    use super::*;

    #[discrim = 0]
    pub fn init_literal(ctx: &mut Context<InitLiteral>) -> Result<()> {
        ctx.accounts.data.bump = ctx.bumps.data;
        Ok(())
    }

    #[discrim = 1]
    pub fn check_literal(_ctx: &mut Context<CheckLiteral>) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn check_literal_explicit_bump(_ctx: &mut Context<CheckLiteralExplicitBump>) -> Result<()> {
        Ok(())
    }

    #[discrim = 3]
    pub fn check_fn_seeds(_ctx: &mut Context<CheckFnSeeds>) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn check_fn_seeds_explicit_bump(
        _ctx: &mut Context<CheckFnSeedsExplicitBump>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn check_const_seeds(_ctx: &mut Context<CheckConstSeeds>) -> Result<()> {
        Ok(())
    }

    #[discrim = 7]
    pub fn init_mixed(ctx: &mut Context<InitMixed>) -> Result<()> {
        ctx.accounts.data.bump = ctx.bumps.data;
        Ok(())
    }

    #[discrim = 8]
    pub fn check_mixed(_ctx: &mut Context<CheckMixed>) -> Result<()> {
        Ok(())
    }

    #[discrim = 9]
    pub fn init_direct_account_field_seed(
        _ctx: &mut Context<InitDirectAccountFieldSeed>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 10]
    pub fn init_wrapped_account_field_seed(
        _ctx: &mut Context<InitWrappedAccountFieldSeed>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 11]
    pub fn init_wrapped_arg_seed(
        _ctx: &mut Context<InitWrappedArgSeed>,
        _next_oracle_id: u64,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 12]
    pub fn check_literal_untrusted_bump(
        _ctx: &mut Context<CheckLiteralUntrustedBump>,
        _untrusted_bump: u8,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 13]
    pub fn check_fn_seeds_untrusted_bump(
        _ctx: &mut Context<CheckFnSeedsUntrustedBump>,
        _untrusted_bump: u8,
    ) -> Result<()> {
        Ok(())
    }
}

// -- Accounts structs --------------------------------------------------------

// 1. Init + literal seeds
#[derive(Accounts)]
pub struct InitLiteral {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [b"data"], bump)]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

// 2. Verify literal seeds + bare bump
#[derive(Accounts)]
pub struct CheckLiteral {
    pub payer: Signer,
    #[account(seeds = [b"data"], bump)]
    pub data: Account<Data>,
}

// 3. Verify literal seeds + explicit bump
#[derive(Accounts)]
pub struct CheckLiteralExplicitBump {
    pub payer: Signer,
    #[account(seeds = [b"data"], bump = data.bump)]
    pub data: Account<Data>,
}

// 4. Verify function-call seeds + bare bump
#[derive(Accounts)]
pub struct CheckFnSeeds {
    pub payer: Signer,
    #[account(seeds = tag_seeds(), bump)]
    pub data: Account<Data>,
}

// 5. Verify function-call seeds + explicit bump
#[derive(Accounts)]
pub struct CheckFnSeedsExplicitBump {
    pub payer: Signer,
    #[account(seeds = tag_seeds(), bump = data.bump)]
    pub data: Account<Data>,
}

// 5b. Optional account + function-call seeds + explicit bump.
//
// Exercises the codegen branch in `derive/src/parse.rs` that handles
// opaque seed expressions paired with `bump = <expr>`. The synthesized
// `Bumps` struct types optional-account slots as `Option<u8>`, so the
// generated assignment must wrap the bump in `Some(...)` to type-check.
#[derive(Accounts)]
pub struct CheckFnSeedsExplicitBumpOptional {
    pub payer: Signer,
    #[account(seeds = tag_seeds(), bump = 0)]
    pub data: Option<Account<Data>>,
}

// 5c. Verify function-call seeds + explicit untrusted bump on UncheckedAccount.
#[derive(Accounts)]
#[instruction(untrusted_bump: u8)]
pub struct CheckFnSeedsUntrustedBump {
    pub payer: Signer,
    #[account(seeds = tag_seeds(), bump = untrusted_bump)]
    pub data: UncheckedAccount,
}

// 6. Verify const-item seeds + bare bump
#[derive(Accounts)]
pub struct CheckConstSeeds {
    pub payer: Signer,
    #[account(seeds = CONST_TAG_SEEDS, bump)]
    pub data: Account<Data>,
}

// 6b. Verify literal seeds + explicit untrusted bump on UncheckedAccount.
#[derive(Accounts)]
#[instruction(untrusted_bump: u8)]
pub struct CheckLiteralUntrustedBump {
    pub payer: Signer,
    #[account(seeds = [b"data"], bump = untrusted_bump)]
    pub data: UncheckedAccount,
}

// 7. Init + mixed seeds (literal + field ref)
#[derive(Accounts)]
pub struct InitMixed {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [b"user", payer.address().as_ref()], bump)]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

// 8. Verify mixed seeds + bare bump
#[derive(Accounts)]
pub struct CheckMixed {
    pub payer: Signer,
    #[account(seeds = [b"user", payer.address().as_ref()], bump)]
    pub data: Account<Data>,
}

// 9. Init + account data field seed.
//
// The IDL/client seed classifier should not confuse this with a manager pubkey
// seed. Runtime validation still evaluates the original account field expr.
#[derive(Accounts)]
pub struct InitDirectAccountFieldSeed {
    #[account(mut)]
    pub payer: Signer,
    pub manager: Account<Manager>,
    #[account(
        init,
        payer = payer,
        seeds = [b"oracle-direct", manager.next_oracle_id.to_le_bytes()],
        bump
    )]
    pub oracle: Account<Oracle>,
    pub system_program: Program<System>,
}

// 10. Init + wrapped account data field seed.
#[derive(Accounts)]
pub struct InitWrappedAccountFieldSeed {
    #[account(mut)]
    pub payer: Signer,
    pub manager: Account<Manager>,
    #[account(
        init,
        payer = payer,
        seeds = [b"oracle-wrapped-account", u64::from(manager.next_oracle_id).to_le_bytes()],
        bump
    )]
    pub oracle: Account<Oracle>,
    pub system_program: Program<System>,
}

// 11. Init + wrapped instruction arg seed.
#[derive(Accounts)]
#[instruction(next_oracle_id: u64)]
pub struct InitWrappedArgSeed {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"oracle-wrapped-arg", u64::from(next_oracle_id).to_le_bytes()],
        bump
    )]
    pub oracle: Account<Oracle>,
    pub system_program: Program<System>,
}
