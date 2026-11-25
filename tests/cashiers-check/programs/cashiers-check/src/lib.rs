//! A cashiers check example. The funds are immediately withdrawn from a user's
//! account and sent to a program controlled `Check` account, where the funds
//! reside until they are "cashed" by the intended recipient. The creator of
//! the check can cancel the check at any time to get back the funds.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount, Transfer};
use std::convert::Into;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod cashiers_check {
    use super::*;

    #[access_control(CreateCheck::accounts(&ctx, nonce))]
    pub fn create_check(
        ctx: Context<CreateCheck>,
        amount: u64,
        memo: Option<String>,
        nonce: u8,
    ) -> Result<()> {
        // Transfer funds to the check.
        let cpi_program_id = ctx.accounts.token_program.key();
        let cpi_accounts = Transfer {
            from: ctx.accounts.from.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Print the check.
        let check = &mut ctx.accounts.check;
        check.amount = amount;
        check.from = *ctx.accounts.from.to_account_info().key;
        check.to = *ctx.accounts.to.to_account_info().key;
        check.vault = *ctx.accounts.vault.to_account_info().key;
        check.nonce = nonce;
        check.memo = memo;

        Ok(())
    }

    #[access_control(not_burned(&ctx.accounts.check))]
    pub fn cash_check(ctx: Context<CashCheck>) -> Result<()> {
        let seeds = &[
            ctx.accounts.check.to_account_info().key.as_ref(),
            &[ctx.accounts.check.nonce],
        ];
        let signer = &[&seeds[..]];
        let cpi_program_id = ctx.accounts.token_program.key();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.to.to_account_info(),
            authority: ctx.accounts.check_signer.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::transfer(cpi_ctx, ctx.accounts.check.amount)?;
        // Burn the check for one time use.
        ctx.accounts.check.burned = true;
        Ok(())
    }

    #[access_control(not_burned(&ctx.accounts.check))]
    pub fn cancel_check(ctx: Context<CancelCheck>) -> Result<()> {
        let seeds = &[
            ctx.accounts.check.to_account_info().key.as_ref(),
            &[ctx.accounts.check.nonce],
        ];
        let signer = &[&seeds[..]];
        let cpi_program_id = ctx.accounts.token_program.key();
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.from.to_account_info(),
            authority: ctx.accounts.check_signer.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::transfer(cpi_ctx, ctx.accounts.check.amount)?;
        ctx.accounts.check.burned = true;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateCheck<'info> {
    // Check being created.
    #[account(zero)]
    check: Account<'info, Check>,
    // Check's token vault.
    #[account(mut, constraint = &vault.owner == check_signer.key)]
    vault: Account<'info, TokenAccount>,
    // Program derived address for the check.
    check_signer: UncheckedAccount<'info>,
    // Token account the check is made from.
    #[account(mut, has_one = owner)]
    from: Account<'info, TokenAccount>,
    // Token account the check is made to.
    #[account(constraint = from.mint == to.mint)]
    to: Account<'info, TokenAccount>,
    // Owner of the `from` token account.
    owner: UncheckedAccount<'info>,
    token_program: UncheckedAccount<'info>,
}

impl<'info> CreateCheck<'info> {
    pub fn accounts(ctx: &Context<CreateCheck>, nonce: u8) -> Result<()> {
        let signer = Pubkey::create_program_address(
            &[ctx.accounts.check.to_account_info().key.as_ref(), &[nonce]],
            ctx.program_id,
        )
        .map_err(|_| error!(ErrorCode::InvalidCheckNonce))?;
        if &signer != ctx.accounts.check_signer.to_account_info().key {
            return err!(ErrorCode::InvalidCheckSigner);
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CashCheck<'info> {
    #[account(mut, has_one = vault, has_one = to)]
    check: Account<'info, Check>,
    #[account(mut)]
    vault: UncheckedAccount<'info>,
    #[account(
        seeds = [check.to_account_info().key.as_ref()],
        bump = check.nonce,
    )]
    check_signer: UncheckedAccount<'info>,
    #[account(mut, has_one = owner)]
    to: Account<'info, TokenAccount>,
    owner: Signer<'info>,
    token_program: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CancelCheck<'info> {
    #[account(mut, has_one = vault, has_one = from)]
    check: Account<'info, Check>,
    #[account(mut)]
    vault: UncheckedAccount<'info>,
    #[account(
        seeds = [check.to_account_info().key.as_ref()],
        bump = check.nonce,
    )]
    check_signer: UncheckedAccount<'info>,
    #[account(mut, has_one = owner)]
    from: Account<'info, TokenAccount>,
    owner: Signer<'info>,
    token_program: UncheckedAccount<'info>,
}

#[account]
pub struct Check {
    from: Pubkey,
    to: Pubkey,
    amount: u64,
    memo: Option<String>,
    vault: Pubkey,
    nonce: u8,
    burned: bool,
}

#[error_code]
pub enum ErrorCode {
    #[msg("The given nonce does not create a valid program derived address.")]
    InvalidCheckNonce,
    #[msg("The derived check signer does not match that which was given.")]
    InvalidCheckSigner,
    #[msg("The given check has already been burned.")]
    AlreadyBurned,
}

fn not_burned(check: &Check) -> Result<()> {
    if check.burned {
        return err!(ErrorCode::AlreadyBurned);
    }
    Ok(())
}
