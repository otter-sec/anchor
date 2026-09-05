//! Examples for `anchor-spl::token_interface` accounts and CPIs.

use {
    anchor_lang::prelude::*,
    anchor_spl::{
        mint,
        token::{self, MintTo},
        token_interface::{Mint, TokenAccount, TokenInterface},
    },
};

declare_id!("79t3uDwfPMnJEgybg7XzsLd54wDyrhskVwhgnmjkRAXj");

#[program]
pub mod token_interface_test {
    use super::*;

    #[discrim = 0]
    pub fn check_token_program(_ctx: &mut Context<CheckTokenProgram>) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn init_interface_mint(_ctx: &mut Context<InitInterfaceMint>) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn init_interface_token_account(
        _ctx: &mut Context<InitInterfaceTokenAccount>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 3]
    pub fn check_interface_token_constraints(
        _ctx: &mut Context<CheckInterfaceTokenConstraints>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn check_interface_mint_constraints(
        _ctx: &mut Context<CheckInterfaceMintConstraints>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn mint_to_interface_account(
        ctx: &mut Context<MintToInterfaceAccount>,
        amount: u64,
    ) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.address(),
            MintTo {
                mint: ctx.accounts.mint.cpi_handle_mut(),
                to: ctx.accounts.to.cpi_handle_mut(),
                authority: ctx.accounts.authority.cpi_handle(),
            },
        );
        token::mint_to(cpi_ctx, amount)?;
        Ok(())
    }

    #[discrim = 6]
    pub fn init_interface_mint_decimals_9(
        _ctx: &mut Context<InitInterfaceMintDecimals9>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 7]
    pub fn init_interface_mint_with_freeze_authority(
        _ctx: &mut Context<InitInterfaceMintWithFreezeAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 8]
    pub fn check_interface_mint_freeze_authority(
        _ctx: &mut Context<CheckInterfaceMintFreezeAuthority>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 9]
    pub fn init_interface_mint_pda(_ctx: &mut Context<InitInterfaceMintPda>) -> Result<()> {
        Ok(())
    }

    #[discrim = 10]
    pub fn init_interface_token_account_pda(
        _ctx: &mut Context<InitInterfaceTokenAccountPda>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CheckTokenProgram {
    pub token_program: Interface<TokenInterface>,
}

#[derive(Accounts)]
pub struct InitInterfaceMint {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<Mint>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitInterfaceMintPda {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        init,
        payer = payer,
        seeds = [b"interface-mint", authority.address().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<Mint>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitInterfaceTokenAccount {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<Mint>,
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<TokenAccount>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitInterfaceTokenAccountPda {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<Mint>,
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        init,
        payer = payer,
        seeds = [
            b"interface-token-account",
            mint.account().address().as_ref(),
            authority.address().as_ref()
        ],
        bump,
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<TokenAccount>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CheckInterfaceTokenConstraints {
    pub mint: InterfaceAccount<Mint>,
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        token::mint = mint,
        token::authority = authority,
        token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<TokenAccount>,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintConstraints {
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        mint::decimals = 6,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<Mint>,
}

#[derive(Accounts)]
pub struct MintToInterfaceAccount {
    #[account(mut, mint::authority = authority, mint::token_program = token_program)]
    pub mint: InterfaceAccount<Mint>,
    #[account(mut, token::mint = mint, token::token_program = token_program)]
    pub to: InterfaceAccount<TokenAccount>,
    pub authority: Signer,
    pub token_program: Interface<TokenInterface>,
}

#[derive(Accounts)]
pub struct InitInterfaceMintDecimals9 {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        init,
        payer = payer,
        mint::decimals = 9,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<Mint>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitInterfaceMintWithFreezeAuthority {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    pub freeze_authority: UncheckedAccount,
    pub token_program: Interface<TokenInterface>,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
        mint::freeze_authority = freeze_authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<Mint>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintFreezeAuthority {
    pub expected: UncheckedAccount,
    #[account(mint::freeze_authority = expected)]
    pub mint: InterfaceAccount<Mint>,
}
