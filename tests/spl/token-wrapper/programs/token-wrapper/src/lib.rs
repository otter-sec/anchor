//! An example program where users can wrap tokens
//! Source tokens are deposited into a vault in exchange for wrapped tokens at a 1:1 ratio
//! This is a stateless implementation which relies on PDAs for security
//!
//! Initializer initializes a new wrapper:
//! - SPL token/token-2022 mint (X) the deposit tokens the wrapper will receive
//! - SPL token/token-2022 mint (Y) the wrapped tokens returned for each deposit token
//!
//! Once this wrapper is initialised:
//! 1. Users can call the wrap function to deposit X and mint Y wrapped tokens
//! 2. Users can call the unwrap function to burn Y and withdraw X unwrapped tokens

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

declare_id!("4ZPcGU8MX8oL2u1EtErHzixAbgNBNeE9yoYq3kKMqnAy");

#[program]
pub mod token_wrapper {
    use super::*;

    pub const WRAPPER_AUTH_SEED: &[u8] = b"wrapr";
    pub const WRAPPER_VAULT_SEED: &[u8] = b"vault";

    pub fn initialize(ctx: Context<Initialize>, initializer_amount: u64) -> Result<()> {
        // deposit into vault
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.deposit_token_program.key(),
                token_interface::TransferChecked {
                    from: ctx
                        .accounts
                        .initializer_deposit_token_account
                        .to_account_info(),
                    mint: ctx.accounts.deposit_mint.to_account_info(),
                    to: ctx.accounts.deposit_token_vault.to_account_info(),
                    authority: ctx.accounts.initializer.to_account_info(),
                },
            ),
            initializer_amount,
            ctx.accounts.deposit_mint.decimals,
        )?;

        // mint wrapped tokens
        let inner_seeds = [
            WRAPPER_AUTH_SEED,
            ctx.accounts.deposit_mint.to_account_info().key.as_ref(),
            ctx.accounts.wrapped_mint.to_account_info().key.as_ref(),
            &[ctx.bumps.wrapper_authority],
        ];
        let signer_seeds = &[&inner_seeds[..]];
        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.wrapped_token_program.key(),
                token_interface::MintTo {
                    mint: ctx.accounts.wrapped_mint.to_account_info(),
                    to: ctx
                        .accounts
                        .initializer_wrapped_token_account
                        .to_account_info(),
                    authority: ctx.accounts.wrapper_authority.to_account_info(),
                },
                signer_seeds,
            ),
            initializer_amount,
        )?;

        Ok(())
    }

    pub fn wrap(ctx: Context<Wrap>, wrap_amount: u64) -> Result<()> {
        // deposit into vault
        token_interface::transfer_checked(
            CpiContext::new(
                ctx.accounts.deposit_token_program.key(),
                token_interface::TransferChecked {
                    from: ctx.accounts.user_deposit_token_account.to_account_info(),
                    mint: ctx.accounts.deposit_mint.to_account_info(),
                    to: ctx.accounts.deposit_token_vault.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            wrap_amount,
            ctx.accounts.deposit_mint.decimals,
        )?;

        // mint wrapped tokens
        let inner_seeds = [
            WRAPPER_AUTH_SEED,
            ctx.accounts.deposit_mint.to_account_info().key.as_ref(),
            ctx.accounts.wrapped_mint.to_account_info().key.as_ref(),
            &[ctx.bumps.wrapper_authority],
        ];
        let signer_seeds = &[&inner_seeds[..]];
        token_interface::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.wrapped_token_program.key(),
                token_interface::MintTo {
                    mint: ctx.accounts.wrapped_mint.to_account_info(),
                    to: ctx.accounts.user_wrapped_token_account.to_account_info(),
                    authority: ctx.accounts.wrapper_authority.to_account_info(),
                },
                signer_seeds,
            ),
            wrap_amount,
        )?;

        Ok(())
    }

    pub fn unwrap(ctx: Context<Unwrap>, unwrap_amount: u64) -> Result<()> {
        // burn wrapped tokens
        token_interface::burn(
            CpiContext::new(
                ctx.accounts.wrapped_token_program.key(),
                token_interface::Burn {
                    mint: ctx.accounts.wrapped_mint.to_account_info(),
                    from: ctx.accounts.user_wrapped_token_account.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info(),
                },
            ),
            unwrap_amount,
        )?;

        // withdraw from vault
        let inner_seeds = [
            WRAPPER_AUTH_SEED,
            ctx.accounts.deposit_mint.to_account_info().key.as_ref(),
            ctx.accounts.wrapped_mint.to_account_info().key.as_ref(),
            &[ctx.bumps.wrapper_authority],
        ];
        let signer_seeds = &[&inner_seeds[..]];
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.deposit_token_program.key(),
                token_interface::TransferChecked {
                    from: ctx.accounts.deposit_token_vault.to_account_info(),
                    mint: ctx.accounts.deposit_mint.to_account_info(),
                    to: ctx.accounts.user_deposit_token_account.to_account_info(),
                    authority: ctx.accounts.wrapper_authority.to_account_info(),
                },
                signer_seeds,
            ),
            unwrap_amount,
            ctx.accounts.deposit_mint.decimals,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(initializer_amount: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,

    #[account(
        mint::token_program = deposit_token_program,
    )]
    pub deposit_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init,
        payer = initializer,
        mint::decimals = deposit_mint.decimals,
        mint::authority = wrapper_authority,
        mint::token_program = wrapped_token_program,
    )]
    pub wrapped_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Program-owned vault to store deposited tokens
    #[account(init,
        seeds = [WRAPPER_VAULT_SEED, deposit_mint.key().as_ref(), wrapped_mint.key().as_ref()],
        bump,
        payer = initializer,
        token::mint = deposit_mint,
        token::authority = wrapper_authority,
        token::token_program = deposit_token_program,
    )]
    pub deposit_token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User token account to deposit tokens from
    #[account(mut,
        constraint = initializer_deposit_token_account.amount >= initializer_amount,
        token::mint = deposit_mint,
        token::authority = initializer,
        token::token_program = deposit_token_program,
    )]
    pub initializer_deposit_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// User token account to send wrapped tokens to
    #[account(init,
        payer = initializer,
        token::mint = wrapped_mint,
        token::authority = initializer,
        token::token_program = wrapped_token_program,
    )]
    pub initializer_wrapped_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: PDA owned by the program
    #[account(mut,
        seeds = [WRAPPER_AUTH_SEED, deposit_mint.key().as_ref(), wrapped_mint.key().as_ref()],
        bump,
    )]
    pub wrapper_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub deposit_token_program: Interface<'info, TokenInterface>,
    pub wrapped_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(wrap_amount: u64)]
pub struct Wrap<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mint::token_program = deposit_token_program,
    )]
    pub deposit_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        mint::authority = wrapper_authority,
        mint::token_program = wrapped_token_program,
    )]
    pub wrapped_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        seeds = [WRAPPER_VAULT_SEED, deposit_mint.key().as_ref(), wrapped_mint.key().as_ref()],
        bump,
        token::mint = deposit_mint,
        token::authority = wrapper_authority,
        token::token_program = deposit_token_program,
    )]
    pub deposit_token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        constraint = user_deposit_token_account.amount >= wrap_amount,
        token::mint = deposit_mint,
        token::token_program = deposit_token_program,
    )]
    pub user_deposit_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = wrapped_mint,
        token::token_program = wrapped_token_program,
    )]
    pub user_wrapped_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: PDA owned by the program
    #[account(mut,
        seeds = [WRAPPER_AUTH_SEED, deposit_mint.key().as_ref(), wrapped_mint.key().as_ref()],
        bump,
    )]
    pub wrapper_authority: UncheckedAccount<'info>,

    pub deposit_token_program: Interface<'info, TokenInterface>,
    pub wrapped_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(unwrap_amount: u64)]
pub struct Unwrap<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        mint::token_program = deposit_token_program,
    )]
    pub deposit_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        mint::token_program = wrapped_token_program,
        mint::authority = wrapper_authority,
    )]
    pub wrapped_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        seeds = [WRAPPER_VAULT_SEED, deposit_mint.key().as_ref(), wrapped_mint.key().as_ref()],
        bump,
        token::mint = deposit_mint,
        token::authority = wrapper_authority,
        token::token_program = deposit_token_program,
    )]
    pub deposit_token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = deposit_mint,
        token::token_program = deposit_token_program,
    )]
    pub user_deposit_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        constraint = user_wrapped_token_account.amount >= unwrap_amount,
        token::mint = wrapped_mint,
        token::token_program = wrapped_token_program,
    )]
    pub user_wrapped_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: PDA owned by the program
    #[account(mut,
        seeds = [crate::token_wrapper::WRAPPER_AUTH_SEED, deposit_mint.key().as_ref(), wrapped_mint.key().as_ref()],
        bump,
    )]
    pub wrapper_authority: UncheckedAccount<'info>,

    pub deposit_token_program: Interface<'info, TokenInterface>,
    pub wrapped_token_program: Interface<'info, TokenInterface>,
}
