use {
    anchor_lang::prelude::*,
    anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface},
};

declare_id!("Fg6PaFpoGXkYsidMpWxTWqkZ6W2BeZ7FEfcYkgMQhgVu");

#[program]
pub mod escrow {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, amount: u64) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow;
        escrow.maker = ctx.accounts.maker.key();
        escrow.mint = ctx.accounts.mint.key();
        escrow.amount = amount;
        escrow.bump = ctx.bumps.escrow;
        Ok(())
    }

    pub fn take(_ctx: Context<Take>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub maker: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        token::mint = mint,
        token::authority = maker,
    )]
    pub maker_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        init,
        payer = maker,
        space = 8 + 32 + 32 + 8 + 1,
        seeds = [b"escrow", maker.key().as_ref(), mint.key().as_ref()],
        bump,
    )]
    pub escrow: Account<'info, Escrow>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Take<'info> {
    pub taker: Signer<'info>,
    #[account(mut)]
    pub maker: SystemAccount<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        token::mint = mint,
        token::authority = maker,
    )]
    pub maker_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        token::mint = mint,
        token::authority = taker,
    )]
    pub taker_token: InterfaceAccount<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"escrow", maker.key().as_ref(), mint.key().as_ref()],
        bump = escrow.bump,
        has_one = maker,
        has_one = mint,
        close = maker,
        constraint = escrow.amount > 0,
    )]
    pub escrow: Account<'info, Escrow>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[account]
pub struct Escrow {
    pub maker: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub bump: u8,
}
