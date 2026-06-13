//! The typescript example serves to show how one would setup an Anchor
//! workspace with TypeScript tests and migrations.

mod other;

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

pub const MY_SEED: [u8; 2] = *b"hi";
pub const MY_SEED_BYTES: &[u8] = b"hi";
pub const MY_SEED_STR: &str = "hi";
pub const MY_SEED_U8: u8 = 1;
pub const MY_SEED_U32: u32 = 2;
pub const MY_SEED_U64: u64 = 3;

#[program]
pub mod pda_derivation {
    use super::*;

    pub fn init_base(ctx: Context<InitBase>, data: u64, data_key: Pubkey) -> Result<()> {
        let base = &mut ctx.accounts.base;
        base.base_data = data;
        base.base_data_key = data_key;
        Ok(())
    }

    pub fn init_another(ctx: Context<InitAnotherBase>, data: u64) -> Result<()> {
        let base = &mut ctx.accounts.base;
        base.data = data;
        Ok(())
    }

    pub fn init_my_account(ctx: Context<InitMyAccount>, _seed_a: u8) -> Result<()> {
        ctx.accounts.account.data = 1337;
        Ok(())
    }

    pub fn test_seed_constant(_ctx: Context<TestSeedConstant>) -> Result<()> {
        Ok(())
    }

    pub fn associated_token_resolution(_ctx: Context<AssociatedTokenResolution>) -> Result<()> {
        Ok(())
    }

    pub fn seed_math_expr(_ctx: Context<SeedMathExpr>) -> Result<()> {
        Ok(())
    }

    pub fn resolution_error(_ctx: Context<ResolutionError>) -> Result<()> {
        Ok(())
    }

    pub fn unsupported_program_seed(_ctx: Context<UnsupportedProgramSeed>) -> Result<()> {
        Ok(())
    }

    pub fn call_expr_with_no_args(_ctx: Context<CallExprWithNoArgs>) -> Result<()> {
        Ok(())
    }

    pub fn pubkey_const(_ctx: Context<PubkeyConst>) -> Result<()> {
        Ok(())
    }

    pub fn seeds_program_account(_ctx: Context<SeedsProgramAccount>) -> Result<()> {
        Ok(())
    }

    pub fn seeds_program_arg(_ctx: Context<SeedsProgramArg>, _some_program: Pubkey) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitBase<'info> {
    #[account(
        init,
        payer = payer,
        space = 8+8+32,
    )]
    base: Account<'info, BaseAccount>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitAnotherBase<'info> {
    #[account(
        init,
        payer = payer,
        space = 8+8,
    )]
    base: Account<'info, crate::other::AnotherBaseAccount>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(seed_a: u8)]
pub struct InitMyAccount<'info> {
    base: Account<'info, BaseAccount>,
    // Intentionally using this qualified form instead of importing to test parsing
    another_base: Account<'info, crate::other::AnotherBaseAccount>,
    base2: UncheckedAccount<'info>,
    #[account(
        init,
        payer = payer,
        space = 8+8,
        seeds = [
            &seed_a.to_le_bytes(),
            "another-seed".as_bytes(),
            b"test".as_ref(),
            base.key().as_ref(),
            base2.key.as_ref(),
            MY_SEED.as_ref(),
            MY_SEED_STR.as_bytes(),
            MY_SEED_U8.to_le_bytes().as_ref(),
            &MY_SEED_U32.to_le_bytes(),
            &MY_SEED_U64.to_le_bytes(),
            base.base_data.to_le_bytes().as_ref(),
            base.base_data_key.as_ref(),
            another_base.data.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    account: Account<'info, MyAccount>,
    nested: Nested<'info>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Nested<'info> {
    #[account(
        seeds = [
            "nested-seed".as_bytes(),
            b"test".as_ref(),
            MY_SEED.as_ref(),
            MY_SEED_STR.as_bytes(),
            MY_SEED_U8.to_le_bytes().as_ref(),
            &MY_SEED_U32.to_le_bytes(),
            &MY_SEED_U64.to_le_bytes(),
        ],
        bump,
    )]
    /// CHECK: Not needed
    account_nested: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct TestSeedConstant<'info> {
    #[account(mut)]
    my_account: Signer<'info>,
    #[account(
      init,
      payer = my_account,
      seeds = [MY_SEED_BYTES],
      space = 100,
      bump,
    )]
    account: Account<'info, MyAccount>,
    system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AssociatedTokenResolution<'info> {
    #[account(
        init,
        payer = payer,
        mint::authority = payer,
        mint::decimals = 9,
    )]
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = payer,
        associated_token::authority = payer,
        associated_token::mint = mint,
    )]
    pub ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
pub struct SeedMathExpr<'info> {
    #[account(seeds = [b"const"], bump)]
    pub my_account: Account<'info, MyAccount>,
    #[account(seeds = [&(my_account.data + 1).to_le_bytes()], bump)]
    pub math_expr_account: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ResolutionError<'info> {
    pub unknown: UncheckedAccount<'info>,
    #[account(seeds = [unknown.key.as_ref()], bump)]
    pub pda: UncheckedAccount<'info>,
    #[account(seeds = [pda.key.as_ref()], bump)]
    pub another_pda: UncheckedAccount<'info>,
}

#[derive(Accounts)]

pub struct UnsupportedProgramSeed<'info> {
    #[account(
        seeds = [],
        seeds::program = external_function_with_an_argument(&pda.key),
        bump
    )]
    pub pda: UncheckedAccount<'info>,
}

fn external_function_with_an_argument(pk: &Pubkey) -> Pubkey {
    *pk
}

#[derive(Accounts)]
pub struct CallExprWithNoArgs<'info> {
    #[account(
        seeds = [System::id().as_ref()],
        seeds::program = System::id(),
        bump
    )]
    pub pda: UncheckedAccount<'info>,
}

const PUBKEY_CONST: Pubkey = pubkey!("4LVUJzLugULF1PemZ1StknKJEEtJM6rJZaGijpNqCouG");

#[derive(Accounts)]
pub struct PubkeyConst<'info> {
    #[account(
        seeds = [],
        seeds::program = PUBKEY_CONST,
        bump
    )]
    pub acc: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SeedsProgramAccount<'info> {
    #[account(
        seeds = [b"*"],
        seeds::program = some_program,
        bump
    )]
    pub pda: UncheckedAccount<'info>,
    pub some_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(some_program: Pubkey)]
pub struct SeedsProgramArg<'info> {
    #[account(
        seeds = [b"*"],
        seeds::program = some_program,
        bump
    )]
    pub pda: UncheckedAccount<'info>,
}

#[account]
pub struct MyAccount {
    data: u64,
}

#[account]
pub struct BaseAccount {
    base_data: u64,
    base_data_key: Pubkey,
}
