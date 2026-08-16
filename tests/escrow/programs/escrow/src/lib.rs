//! An example of an escrow program, inspired by PaulX tutorial seen here
//! https://paulx.dev/blog/2021/01/14/programming-on-solana-an-introduction/
//! This example has some changes to implementation, but more or less should be the same overall
//! Also gives examples on how to use some newer anchor features and CPI
//!
//! User (Initializer) constructs an escrow deal:
//! - SPL token (X) they will offer and amount
//! - SPL token (Y) count they want in return and amount
//! - Program will take ownership of initializer's token X account
//!
//! Once this escrow is initialised, either:
//! 1. User (Taker) can call the exchange function to exchange their Y for X
//! - This will close the escrow account and no longer be usable
//! OR
//! 2. If no one has exchanged, the initializer can close the escrow account
//! - Initializer will get back ownership of their token X account

use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::spl_token_2022::instruction::AuthorityType,
    token_interface::{self, Mint, SetAuthority, TokenAccount, TokenInterface, TransferChecked},
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod escrow {
    use super::*;

    const ESCROW_PDA_SEED: &[u8] = b"escrow";

    pub fn initialize_escrow(
        ctx: Context<InitializeEscrow>,
        initializer_amount: u64,
        taker_amount: u64,
    ) -> Result<()> {
        ctx.accounts.escrow_account.initializer_key = *ctx.accounts.initializer.key;
        ctx.accounts
            .escrow_account
            .initializer_deposit_token_account = *ctx
            .accounts
            .initializer_deposit_token_account
            .to_account_info()
            .key;
        ctx.accounts
            .escrow_account
            .initializer_receive_token_account = *ctx
            .accounts
            .initializer_receive_token_account
            .to_account_info()
            .key;
        ctx.accounts.escrow_account.initializer_amount = initializer_amount;
        ctx.accounts.escrow_account.taker_amount = taker_amount;

        let (pda, _bump_seed) = Pubkey::find_program_address(&[ESCROW_PDA_SEED], ctx.program_id);
        token_interface::set_authority(
            ctx.accounts.into(),
            AuthorityType::AccountOwner,
            Some(pda),
        )?;
        Ok(())
    }

    pub fn cancel_escrow(ctx: Context<CancelEscrow>) -> Result<()> {
        let (_pda, bump_seed) = Pubkey::find_program_address(&[ESCROW_PDA_SEED], ctx.program_id);
        let seeds = &[&ESCROW_PDA_SEED[..], &[bump_seed]];

        token_interface::set_authority(
            ctx.accounts
                .into_set_authority_context()
                .with_signer(&[&seeds[..]]),
            AuthorityType::AccountOwner,
            Some(ctx.accounts.escrow_account.initializer_key),
        )?;

        Ok(())
    }

    pub fn exchange(ctx: Context<Exchange>) -> Result<()> {
        // Transferring from initializer to taker
        let (_pda, bump_seed) = Pubkey::find_program_address(&[ESCROW_PDA_SEED], ctx.program_id);
        let seeds = &[&ESCROW_PDA_SEED[..], &[bump_seed]];

        token_interface::transfer_checked(
            ctx.accounts
                .into_transfer_to_taker_context()
                .with_signer(&[&seeds[..]]),
            ctx.accounts.escrow_account.initializer_amount,
            ctx.accounts.receive_mint.decimals,
        )?;

        token_interface::transfer_checked(
            ctx.accounts.into_transfer_to_initializer_context(),
            ctx.accounts.escrow_account.taker_amount,
            ctx.accounts.deposit_mint.decimals,
        )?;

        token_interface::set_authority(
            ctx.accounts
                .into_set_authority_context()
                .with_signer(&[&seeds[..]]),
            AuthorityType::AccountOwner,
            Some(ctx.accounts.escrow_account.initializer_key),
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(initializer_amount: u64)]
pub struct InitializeEscrow<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    #[account(
        mut,
        constraint = initializer_deposit_token_account.amount >= initializer_amount
    )]
    pub initializer_deposit_token_account: InterfaceAccount<'info, TokenAccount>,
    pub initializer_receive_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(init, payer = initializer, space = 8 + EscrowAccount::LEN)]
    pub escrow_account: Account<'info, EscrowAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct Exchange<'info> {
    #[account(signer)]
    /// CHECK:
    pub taker: UncheckedAccount<'info>,
    #[account(mut, token::mint = deposit_mint)]
    pub taker_deposit_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = receive_mint)]
    pub taker_receive_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = receive_mint)]
    pub pda_deposit_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, token::mint = deposit_mint)]
    pub initializer_receive_token_account: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    /// CHECK:
    pub initializer_main_account: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = escrow_account.taker_amount <= taker_deposit_token_account.amount,
        constraint = escrow_account.initializer_deposit_token_account == *pda_deposit_token_account.to_account_info().key,
        constraint = escrow_account.initializer_receive_token_account == *initializer_receive_token_account.to_account_info().key,
        constraint = escrow_account.initializer_key == *initializer_main_account.key,
        close = initializer_main_account
    )]
    pub escrow_account: Account<'info, EscrowAccount>,
    /// CHECK:
    pub pda_account: UncheckedAccount<'info>,
    pub deposit_mint: Box<InterfaceAccount<'info, Mint>>,
    pub receive_mint: Box<InterfaceAccount<'info, Mint>>,
    pub deposit_token_program: Interface<'info, TokenInterface>,
    pub receive_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct CancelEscrow<'info> {
    /// CHECK:
    pub initializer: UncheckedAccount<'info>,
    #[account(mut)]
    pub pda_deposit_token_account: InterfaceAccount<'info, TokenAccount>,
    /// CHECK:
    pub pda_account: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = escrow_account.initializer_key == *initializer.key,
        constraint = escrow_account.initializer_deposit_token_account == *pda_deposit_token_account.to_account_info().key,
        close = initializer
    )]
    pub escrow_account: Account<'info, EscrowAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct EscrowAccount {
    pub initializer_key: Pubkey,
    pub initializer_deposit_token_account: Pubkey,
    pub initializer_receive_token_account: Pubkey,
    pub initializer_amount: u64,
    pub taker_amount: u64,
}

impl EscrowAccount {
    pub const LEN: usize = 32 + 32 + 32 + 8 + 8;
}

impl<'info> From<&mut InitializeEscrow<'info>>
    for CpiContext<'_, '_, '_, 'info, SetAuthority<'info>>
{
    fn from(accounts: &mut InitializeEscrow<'info>) -> Self {
        let cpi_accounts = SetAuthority {
            account_or_mint: accounts.initializer_deposit_token_account.to_account_info(),
            current_authority: accounts.initializer.to_account_info(),
        };
        let cpi_program_id = accounts.token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'info> CancelEscrow<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.pda_deposit_token_account.to_account_info(),
            current_authority: self.pda_account.to_account_info(),
        };
        let cpi_program_id = self.token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'info> Exchange<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.pda_deposit_token_account.to_account_info(),
            current_authority: self.pda_account.to_account_info(),
        };
        let cpi_program_id = self.receive_token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'info> Exchange<'info> {
    fn into_transfer_to_taker_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.pda_deposit_token_account.to_account_info(),
            mint: self.receive_mint.to_account_info(),
            to: self.taker_receive_token_account.to_account_info(),
            authority: self.pda_account.to_account_info(),
        };
        let cpi_program_id = self.receive_token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'info> Exchange<'info> {
    fn into_transfer_to_initializer_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, TransferChecked<'info>> {
        let cpi_accounts = TransferChecked {
            from: self.taker_deposit_token_account.to_account_info(),
            mint: self.deposit_mint.to_account_info(),
            to: self.initializer_receive_token_account.to_account_info(),
            authority: self.taker.to_account_info(),
        };
        let cpi_program_id = self.deposit_token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}
