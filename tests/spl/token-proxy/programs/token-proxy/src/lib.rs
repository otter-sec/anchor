//! This example demonstrates the use of the `anchor_spl::token` CPI client.

use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_2022::spl_token_2022,
    token_interface::{
        self, Burn, Mint, MintTo, SetAuthority, TokenAccount, TokenInterface, Transfer,
        TransferChecked,
    },
};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
mod token_proxy {
    use super::*;

    pub fn proxy_transfer(ctx: Context<ProxyTransfer>, amount: u64) -> Result<()> {
        #[allow(deprecated)]
        token_interface::transfer(ctx.accounts.into(), amount)
    }

    pub fn proxy_optional_transfer(ctx: Context<ProxyOptionalTransfer>, amount: u64) -> Result<()> {
        if let Some(token_program) = &ctx.accounts.token_program {
            if let Some(mint) = &ctx.accounts.mint {
                let cpi_accounts = TransferChecked {
                    from: ctx.accounts.from.to_account_info(),
                    mint: mint.to_account_info(),
                    to: ctx.accounts.to.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                };
                let cpi_program_id = token_program.key();
                let cpi_context = CpiContext::new(cpi_program_id, cpi_accounts);
                token_interface::transfer_checked(cpi_context, amount, mint.decimals)
            } else {
                let cpi_accounts = Transfer {
                    from: ctx.accounts.from.to_account_info(),
                    to: ctx.accounts.to.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                };
                let cpi_program_id = token_program.key();
                let cpi_context = CpiContext::new(cpi_program_id, cpi_accounts);
                #[allow(deprecated)]
                token_interface::transfer(cpi_context, amount)
            }
        } else {
            Ok(())
        }
    }

    pub fn proxy_mint_to(ctx: Context<ProxyMintTo>, amount: u64) -> Result<()> {
        token_interface::mint_to(ctx.accounts.into(), amount)
    }

    pub fn proxy_burn(ctx: Context<ProxyBurn>, amount: u64) -> Result<()> {
        token_interface::burn(ctx.accounts.into(), amount)
    }

    pub fn proxy_set_authority(
        ctx: Context<ProxySetAuthority>,
        authority_type: AuthorityType,
        new_authority: Option<Pubkey>,
    ) -> Result<()> {
        token_interface::set_authority(ctx.accounts.into(), authority_type.into(), new_authority)
    }

    pub fn proxy_create_token_account(_ctx: Context<ProxyCreateTokenAccount>) -> Result<()> {
        Ok(())
    }

    pub fn proxy_create_associated_token_account(
        _ctx: Context<ProxyCreateAssociatedTokenAccount>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn proxy_create_mint(_ctx: Context<ProxyCreateMint>, _name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub enum AuthorityType {
    /// Authority to mint new tokens
    MintTokens,
    /// Authority to freeze any account associated with the Mint
    FreezeAccount,
    /// Owner of a given token account
    AccountOwner,
    /// Authority to close a token account
    CloseAccount,
}

#[derive(Accounts)]
pub struct ProxyTransfer<'info> {
    #[account(signer)]
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ProxyOptionalTransfer<'info> {
    #[account(signer)]
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub mint: Option<InterfaceAccount<'info, Mint>>,
    pub token_program: Option<Interface<'info, TokenInterface>>,
}

#[derive(Accounts)]
pub struct ProxyMintTo<'info> {
    #[account(signer)]
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub to: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ProxyBurn<'info> {
    #[account(signer)]
    /// CHECK:
    pub authority: UncheckedAccount<'info>,
    #[account(mut)]
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub from: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ProxySetAuthority<'info> {
    #[account(signer)]
    /// CHECK:
    pub current_authority: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK:
    pub account_or_mint: UncheckedAccount<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ProxyCreateTokenAccount<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(init,
        token::mint = mint,
        token::authority = authority,
        seeds = [authority.key().as_ref(), mint.key().as_ref(), b"token-proxy-account"],
        bump,
        payer = authority
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ProxyCreateAssociatedTokenAccount<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        associated_token::mint = mint,
        payer = authority,
        associated_token::authority = authority,
    )]
    pub token_account: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct ProxyCreateMint<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        mint::decimals = 9,
        mint::authority = authority,
        seeds = [authority.key().as_ref(), name.as_bytes(), b"token-proxy-mint"],
        bump,
        payer = authority
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub system_program: Program<'info, System>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl<'a, 'b, 'c, 'info> From<&mut ProxyTransfer<'info>>
    for CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>
{
    fn from(accounts: &mut ProxyTransfer<'info>) -> CpiContext<'a, 'b, 'c, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: accounts.from.to_account_info(),
            to: accounts.to.to_account_info(),
            authority: accounts.authority.to_account_info(),
        };
        let cpi_program_id = accounts.token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'a, 'b, 'c, 'info> From<&mut ProxyMintTo<'info>>
    for CpiContext<'a, 'b, 'c, 'info, MintTo<'info>>
{
    fn from(accounts: &mut ProxyMintTo<'info>) -> CpiContext<'a, 'b, 'c, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: accounts.mint.to_account_info(),
            to: accounts.to.to_account_info(),
            authority: accounts.authority.to_account_info(),
        };
        let cpi_program_id = accounts.token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'a, 'b, 'c, 'info> From<&mut ProxyBurn<'info>> for CpiContext<'a, 'b, 'c, 'info, Burn<'info>> {
    fn from(accounts: &mut ProxyBurn<'info>) -> CpiContext<'a, 'b, 'c, 'info, Burn<'info>> {
        let cpi_accounts = Burn {
            mint: accounts.mint.to_account_info(),
            from: accounts.from.to_account_info(),
            authority: accounts.authority.to_account_info(),
        };
        let cpi_program_id = accounts.token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl<'a, 'b, 'c, 'info> From<&mut ProxySetAuthority<'info>>
    for CpiContext<'a, 'b, 'c, 'info, SetAuthority<'info>>
{
    fn from(
        accounts: &mut ProxySetAuthority<'info>,
    ) -> CpiContext<'a, 'b, 'c, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: accounts.account_or_mint.to_account_info(),
            current_authority: accounts.current_authority.to_account_info(),
        }; // TODO: Support multisig signers
        let cpi_program_id = accounts.token_program.key();
        CpiContext::new(cpi_program_id, cpi_accounts)
    }
}

impl From<AuthorityType> for spl_token_2022::instruction::AuthorityType {
    fn from(authority_ty: AuthorityType) -> spl_token_2022::instruction::AuthorityType {
        match authority_ty {
            AuthorityType::MintTokens => spl_token_2022::instruction::AuthorityType::MintTokens,
            AuthorityType::FreezeAccount => {
                spl_token_2022::instruction::AuthorityType::FreezeAccount
            }
            AuthorityType::AccountOwner => spl_token_2022::instruction::AuthorityType::AccountOwner,
            AuthorityType::CloseAccount => spl_token_2022::instruction::AuthorityType::CloseAccount,
        }
    }
}
