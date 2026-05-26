//! Focused associated-token-account coverage for `anchor-spl-v2`.

use {
    anchor_lang_v2::prelude::*,
    anchor_spl_v2::{
        associated_token::{self, AssociatedToken},
        mint::{self, Mint},
        token::{self, MintTo, Token, TokenAccount},
        token_interface,
    },
};

declare_id!("AtA1111111111111111111111111111111111111111");

#[program]
pub mod spl_ata_test {
    use super::*;

    #[discrim = 0]
    pub fn init_mint(_ctx: &mut Context<InitMint>) -> Result<()> {
        Ok(())
    }

    #[discrim = 1]
    pub fn init_token_account(_ctx: &mut Context<InitTokenAccount>) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn init_ata(_ctx: &mut Context<InitAta>) -> Result<()> {
        Ok(())
    }

    #[discrim = 3]
    pub fn init_ata_if_needed(_ctx: &mut Context<InitAtaIfNeeded>) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn validate_ata(_ctx: &mut Context<ValidateAta>) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn validate_ata_with_token_program(
        _ctx: &mut Context<ValidateAtaWithTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 6]
    pub fn init_interface_ata_with_token_program(
        _ctx: &mut Context<InitInterfaceAtaWithTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 7]
    pub fn validate_interface_ata_with_token_program(
        _ctx: &mut Context<ValidateInterfaceAtaWithTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 8]
    pub fn mint_to_ata(ctx: &mut Context<MintToAta>, amount: u64) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.address(),
            MintTo {
                mint: ctx.accounts.mint.cpi_handle_mut(),
                to: ctx.accounts.token_account.cpi_handle_mut(),
                authority: ctx.accounts.authority.cpi_handle(),
            },
        );
        anchor_spl_v2::token::mint_to(cpi_ctx, amount)?;
        Ok(())
    }

    #[discrim = 9]
    pub fn init_interface_mint_with_token_program(
        _ctx: &mut Context<InitInterfaceMintWithTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 10]
    pub fn init_interface_ata_if_needed_with_token_program(
        _ctx: &mut Context<InitInterfaceAtaIfNeededWithTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 11]
    pub fn init_many_associated_token_accounts(
        _ctx: &mut Context<InitManyAssociatedTokenAccounts>,
    ) -> Result<()> {
        Ok(())
    }

    #[discrim = 12]
    pub fn set_ata_owner(ctx: &mut Context<SetAtaOwner>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.address(),
            token::SetAuthority {
                account_or_mint: ctx.accounts.token_account.cpi_handle_mut(),
                current_authority: ctx.accounts.current_authority.cpi_handle(),
            },
        );
        token::set_authority(
            cpi_ctx,
            token::spl_token::instruction::AuthorityType::AccountOwner,
            Some(*ctx.accounts.new_authority.address()),
        )?;
        Ok(())
    }

    #[discrim = 13]
    pub fn direct_create_ata(ctx: &mut Context<DirectCreateAta>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.associated_token_program.address(),
            associated_token::Create {
                payer: ctx.accounts.payer.cpi_handle_mut(),
                associated_token: ctx.accounts.associated_token.cpi_handle_mut(),
                authority: ctx.accounts.authority.cpi_handle(),
                mint: ctx.accounts.mint.cpi_handle(),
                system_program: ctx.accounts.system_program.cpi_handle(),
                token_program: ctx.accounts.token_program.cpi_handle(),
            },
        );
        associated_token::create(cpi_ctx)?;
        Ok(())
    }

    #[discrim = 14]
    pub fn init_strict_ata_with_token_program(
        _ctx: &mut Context<InitStrictAtaWithTokenProgram>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitMint {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
    )]
    pub mint: Account<Mint>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitTokenAccount {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        token::mint = mint,
        token::authority = authority,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitAta {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitAtaIfNeeded {
    #[account(mut)]
    pub payer: Signer,
    pub mint: Account<Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct ValidateAta {
    pub mint: Account<Mint>,
    pub authority: UncheckedAccount,
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct ValidateAtaWithTokenProgram {
    pub mint: Account<Mint>,
    pub authority: UncheckedAccount,
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct InitInterfaceAtaWithTokenProgram {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
    pub token_program: UncheckedAccount,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitStrictAtaWithTokenProgram {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: Account<TokenAccount>,
    pub token_program: UncheckedAccount,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct ValidateInterfaceAtaWithTokenProgram {
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub authority: UncheckedAccount,
    #[account(
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct MintToAta {
    #[account(mut)]
    pub mint: Account<Mint>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = owner,
    )]
    pub token_account: Account<TokenAccount>,
    pub owner: UncheckedAccount,
    pub authority: Signer,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct InitInterfaceMintWithTokenProgram {
    #[account(mut)]
    pub payer: Signer,
    pub authority: UncheckedAccount,
    pub token_program: UncheckedAccount,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = authority,
        mint::token_program = token_program,
    )]
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitInterfaceAtaIfNeededWithTokenProgram {
    #[account(mut)]
    pub payer: Signer,
    pub mint: InterfaceAccount<token_interface::Mint>,
    pub authority: UncheckedAccount,
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority,
        associated_token::token_program = token_program,
    )]
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
    pub token_program: UncheckedAccount,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitManyAssociatedTokenAccounts {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        mint::decimals = 6,
        mint::authority = payer,
    )]
    pub mint: Account<Mint>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
    )]
    pub payer_ata: Account<TokenAccount>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = system_program,
    )]
    pub system_ata: Account<TokenAccount>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = token_program,
    )]
    pub token_program_ata: Account<TokenAccount>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = associated_token_program,
    )]
    pub associated_token_program_ata: Account<TokenAccount>,
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = mint,
    )]
    pub mint_ata: Account<TokenAccount>,
    pub token_program: Program<Token>,
    pub associated_token_program: Program<AssociatedToken>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct SetAtaOwner {
    #[account(mut)]
    pub token_account: Account<TokenAccount>,
    pub current_authority: Signer,
    pub new_authority: UncheckedAccount,
    pub token_program: Program<Token>,
}

#[derive(Accounts)]
pub struct DirectCreateAta {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub associated_token: UncheckedAccount,
    pub authority: UncheckedAccount,
    pub mint: Account<Mint>,
    pub system_program: UncheckedAccount,
    pub token_program: UncheckedAccount,
    pub associated_token_program: UncheckedAccount,
}
