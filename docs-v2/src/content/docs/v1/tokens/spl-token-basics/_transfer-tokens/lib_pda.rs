use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{self, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked},
};

declare_id!("3pX5NKLru1UBDVckynWQxsgnJeUN3N1viy36Gk9TSn8d");

#[program]
pub mod token_example {
    use super::*;

    pub fn create_and_mint_tokens(ctx: Context<CreateAndMintTokens>, amount: u64) -> Result<()> {
        let signer_seeds: &[&[&[u8]]] = &[&[b"mint", &[ctx.bumps.mint]]];

        let cpi_accounts = MintTo {
            mint: ctx.accounts.mint.to_account_info(),
            to: ctx.accounts.token_account.to_account_info(),
            authority: ctx.accounts.mint.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer_seeds);
        token_interface::mint_to(cpi_context, amount)?;
        Ok(())
    }

    pub fn transfer_tokens(ctx: Context<TransferTokens>) -> Result<()> {
        let signer_seeds: &[&[&[u8]]] = &[&[b"token", &[ctx.bumps.sender_token_account]]];

        let amount = ctx.accounts.sender_token_account.amount;
        let decimals = ctx.accounts.mint.decimals;

        let cpi_accounts = TransferChecked {
            mint: ctx.accounts.mint.to_account_info(),
            from: ctx.accounts.sender_token_account.to_account_info(),
            to: ctx.accounts.recipient_token_account.to_account_info(),
            authority: ctx.accounts.sender_token_account.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_context = CpiContext::new(cpi_program, cpi_accounts).with_signer(signer_seeds);
        token_interface::transfer_checked(cpi_context, amount, decimals)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateAndMintTokens<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        payer = signer,
        mint::decimals = 6,
        mint::authority = mint,
        mint::freeze_authority = mint,
        seeds = [b"mint"],
        bump
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        init,
        payer = signer,
        token::mint = mint,
        token::authority = token_account,
        seeds = [b"token"],
        bump
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TransferTokens<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"mint"],
        bump
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(
        mut,
        token::mint = mint,
        token::authority = sender_token_account,
        seeds = [b"token"],
        bump
    )]
    pub sender_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(
        init_if_needed,
        payer = signer,
        associated_token::mint = mint,
        associated_token::authority = signer,
        associated_token::token_program = token_program,
    )]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
