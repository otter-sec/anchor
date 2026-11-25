//! An IDO pool program implementing the Mango Markets token sale design here:
//! https://docs.mango.markets/litepaper#token-sale.
// #![warn(clippy::all)]

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, CloseAccount, Mint, MintTo, Token, TokenAccount, Transfer};

use std::ops::Deref;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const DECIMALS: u8 = 6;

#[program]
pub mod ido_pool {
    use super::*;

    #[access_control(validate_ido_times(ido_times))]
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        ido_name: String,
        _bumps: PoolBumps, // No longer used.
        num_ido_tokens: u64,
        ido_times: IdoTimes,
    ) -> Result<()> {
        msg!("INITIALIZE POOL");

        let ido_account = &mut ctx.accounts.ido_account;

        let name_bytes = ido_name.as_bytes();
        let mut name_data = [b' '; 10];
        name_data[..name_bytes.len()].copy_from_slice(name_bytes);

        ido_account.ido_name = name_data;
        ido_account.bumps = PoolBumps {
            ido_account: ctx.bumps.ido_account,
            redeemable_mint: ctx.bumps.redeemable_mint,
            pool_watermelon: ctx.bumps.pool_watermelon,
            pool_usdc: ctx.bumps.pool_usdc,
        };
        ido_account.ido_authority = ctx.accounts.ido_authority.key();

        ido_account.usdc_mint = ctx.accounts.usdc_mint.key();
        ido_account.redeemable_mint = ctx.accounts.redeemable_mint.key();
        ido_account.watermelon_mint = ctx.accounts.watermelon_mint.key();
        ido_account.pool_usdc = ctx.accounts.pool_usdc.key();
        ido_account.pool_watermelon = ctx.accounts.pool_watermelon.key();

        ido_account.num_ido_tokens = num_ido_tokens;
        ido_account.ido_times = ido_times;

        // Transfer Watermelon from ido_authority to pool account.
        let cpi_program_id = ctx.accounts.token_program.key();
        let cpi_accounts = Transfer {
            from: ctx.accounts.ido_authority_watermelon.to_account_info(),
            to: ctx.accounts.pool_watermelon.to_account_info(),
            authority: ctx.accounts.ido_authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        token::transfer(cpi_ctx, num_ido_tokens)?;

        Ok(())
    }

    #[access_control(unrestricted_phase(&ctx.accounts.ido_account))]
    pub fn init_user_redeemable(ctx: Context<InitUserRedeemable>) -> Result<()> {
        msg!("INIT USER REDEEMABLE");
        Ok(())
    }

    #[access_control(unrestricted_phase(&ctx.accounts.ido_account))]
    pub fn exchange_usdc_for_redeemable(
        ctx: Context<ExchangeUsdcForRedeemable>,
        amount: u64,
    ) -> Result<()> {
        msg!("EXCHANGE USDC FOR REDEEMABLE");
        // While token::transfer will check this, we prefer a verbose err msg.
        if ctx.accounts.user_usdc.amount < amount {
            return err!(ErrorCode::LowUsdc);
        }

        let cpi_program_id = ctx.accounts.token_program.key();

        // Transfer user's USDC to pool USDC account.
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_usdc.to_account_info(),
            to: ctx.accounts.pool_usdc.to_account_info(),
            authority: ctx.accounts.user_authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Mint Redeemable to user Redeemable account.
        let ido_name = ctx.accounts.ido_account.ido_name.as_ref();
        let seeds = &[
            ido_name.trim_ascii_whitespace(),
            &[ctx.accounts.ido_account.bumps.ido_account],
        ];
        let signer = &[&seeds[..]];
        let cpi_accounts = MintTo {
            mint: ctx.accounts.redeemable_mint.to_account_info(),
            to: ctx.accounts.user_redeemable.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::mint_to(cpi_ctx, amount)?;

        Ok(())
    }

    #[access_control(withdraw_phase(&ctx.accounts.ido_account))]
    pub fn init_escrow_usdc(ctx: Context<InitEscrowUsdc>) -> Result<()> {
        msg!("INIT ESCROW USDC");
        Ok(())
    }

    #[access_control(withdraw_phase(&ctx.accounts.ido_account))]
    pub fn exchange_redeemable_for_usdc(
        ctx: Context<ExchangeRedeemableForUsdc>,
        amount: u64,
    ) -> Result<()> {
        msg!("EXCHANGE REDEEMABLE FOR USDC");
        // While token::burn will check this, we prefer a verbose err msg.
        if ctx.accounts.user_redeemable.amount < amount {
            return err!(ErrorCode::LowRedeemable);
        }

        let ido_name = ctx.accounts.ido_account.ido_name.as_ref();
        let seeds = &[
            ido_name.trim_ascii_whitespace(),
            &[ctx.accounts.ido_account.bumps.ido_account],
        ];
        let signer = &[&seeds[..]];
        let cpi_program_id = ctx.accounts.token_program.key();

        // Burn the user's redeemable tokens.
        let cpi_accounts = Burn {
            mint: ctx.accounts.redeemable_mint.to_account_info(),
            from: ctx.accounts.user_redeemable.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::burn(cpi_ctx, amount)?;

        // Transfer USDC from pool account to the user's escrow account.
        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_usdc.to_account_info(),
            to: ctx.accounts.escrow_usdc.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::transfer(cpi_ctx, amount)?;

        Ok(())
    }

    #[access_control(ido_over(&ctx.accounts.ido_account))]
    pub fn exchange_redeemable_for_watermelon(
        ctx: Context<ExchangeRedeemableForWatermelon>,
        amount: u64,
    ) -> Result<()> {
        msg!("EXCHANGE REDEEMABLE FOR WATERMELON");
        // While token::burn will check this, we prefer a verbose err msg.
        if ctx.accounts.user_redeemable.amount < amount {
            return err!(ErrorCode::LowRedeemable);
        }

        // Calculate watermelon tokens due.
        let watermelon_amount = (amount as u128)
            .checked_mul(ctx.accounts.pool_watermelon.amount as u128)
            .unwrap()
            .checked_div(ctx.accounts.redeemable_mint.supply as u128)
            .unwrap();

        let ido_name = ctx.accounts.ido_account.ido_name.as_ref();
        let seeds = &[
            ido_name.trim_ascii_whitespace(),
            &[ctx.accounts.ido_account.bumps.ido_account],
        ];
        let signer = &[&seeds[..]];

        let cpi_program_id = ctx.accounts.token_program.key();

        // Burn the user's redeemable tokens.
        let cpi_accounts = Burn {
            mint: ctx.accounts.redeemable_mint.to_account_info(),
            from: ctx.accounts.user_redeemable.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::burn(cpi_ctx, amount)?;

        // Transfer Watermelon from pool account to user.
        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_watermelon.to_account_info(),
            to: ctx.accounts.user_watermelon.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::transfer(cpi_ctx, watermelon_amount as u64)?;

        // Send rent back to user if account is empty
        ctx.accounts.user_redeemable.reload()?;
        if ctx.accounts.user_redeemable.amount == 0 {
            let cpi_accounts = CloseAccount {
                account: ctx.accounts.user_redeemable.to_account_info(),
                destination: ctx.accounts.user_authority.to_account_info(),
                authority: ctx.accounts.ido_account.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
            token::close_account(cpi_ctx)?;
        }

        Ok(())
    }

    #[access_control(ido_over(&ctx.accounts.ido_account))]
    pub fn withdraw_pool_usdc(ctx: Context<WithdrawPoolUsdc>) -> Result<()> {
        msg!("WITHDRAW POOL USDC");
        // Transfer total USDC from pool account to ido_authority account.
        let ido_name = ctx.accounts.ido_account.ido_name.as_ref();
        let seeds = &[
            ido_name.trim_ascii_whitespace(),
            &[ctx.accounts.ido_account.bumps.ido_account],
        ];
        let signer = &[&seeds[..]];
        let cpi_program_id = ctx.accounts.token_program.key();
        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_usdc.to_account_info(),
            to: ctx.accounts.ido_authority_usdc.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::transfer(cpi_ctx, ctx.accounts.pool_usdc.amount)?;

        Ok(())
    }

    #[access_control(escrow_over(&ctx.accounts.ido_account))]
    pub fn withdraw_from_escrow(ctx: Context<WithdrawFromEscrow>, amount: u64) -> Result<()> {
        msg!("WITHDRAW FROM ESCROW");
        // While token::transfer will check this, we prefer a verbose err msg.
        if ctx.accounts.escrow_usdc.amount < amount {
            return err!(ErrorCode::LowUsdc);
        }

        let ido_name = ctx.accounts.ido_account.ido_name.as_ref();
        let seeds = &[
            ido_name.trim_ascii_whitespace(),
            &[ctx.accounts.ido_account.bumps.ido_account],
        ];
        let signer = &[&seeds[..]];
        let cpi_program_id = ctx.accounts.token_program.key();

        // Transfer USDC from user's escrow account to user's USDC account.
        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_usdc.to_account_info(),
            to: ctx.accounts.user_usdc.to_account_info(),
            authority: ctx.accounts.ido_account.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
        token::transfer(cpi_ctx, amount)?;

        // Send rent back to user if account is empty
        ctx.accounts.escrow_usdc.reload()?;
        if ctx.accounts.escrow_usdc.amount == 0 {
            let cpi_accounts = CloseAccount {
                account: ctx.accounts.escrow_usdc.to_account_info(),
                destination: ctx.accounts.user_authority.to_account_info(),
                authority: ctx.accounts.ido_account.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(cpi_program_id, cpi_accounts, signer);
            token::close_account(cpi_ctx)?;
        }

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(ido_name: String, bumps: PoolBumps)]
pub struct InitializePool<'info> {
    // IDO Authority accounts
    #[account(mut)]
    pub ido_authority: Signer<'info>,
    // Watermelon Doesn't have to be an ATA because it could be DAO controlled
    #[account(mut,
        constraint = ido_authority_watermelon.owner == ido_authority.key(),
        constraint = ido_authority_watermelon.mint == watermelon_mint.key())]
    pub ido_authority_watermelon: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(init,
        seeds = [ido_name.as_bytes()],
        bump,
        payer = ido_authority,
        space = 8 + IdoAccount::INIT_SPACE
    )]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    // TODO Confirm USDC mint address on mainnet or leave open as an option for other stables
    #[account(constraint = usdc_mint.decimals == DECIMALS)]
    pub usdc_mint: Box<Account<'info, Mint>>,
    #[account(init,
        mint::decimals = DECIMALS,
        mint::authority = ido_account,
        seeds = [ido_name.as_bytes(), b"redeemable_mint".as_ref()],
        bump,
        payer = ido_authority
    )]
    pub redeemable_mint: Box<Account<'info, Mint>>,
    #[account(constraint = watermelon_mint.key() == ido_authority_watermelon.mint)]
    pub watermelon_mint: Box<Account<'info, Mint>>,
    #[account(init,
        token::mint = watermelon_mint,
        token::authority = ido_account,
        seeds = [ido_name.as_bytes(), b"pool_watermelon"],
        bump,
        payer = ido_authority
    )]
    pub pool_watermelon: Box<Account<'info, TokenAccount>>,
    #[account(init,
        token::mint = usdc_mint,
        token::authority = ido_account,
        seeds = [ido_name.as_bytes(), b"pool_usdc"],
        bump,
        payer = ido_authority
    )]
    pub pool_usdc: Box<Account<'info, TokenAccount>>,
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct InitUserRedeemable<'info> {
    // User Accounts
    #[account(mut)]
    pub user_authority: Signer<'info>,
    #[account(init,
        token::mint = redeemable_mint,
        token::authority = ido_account,
        seeds = [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"user_redeemable"],
        bump,
        payer = user_authority
    )]
    pub user_redeemable: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"redeemable_mint"],
        bump = ido_account.bumps.redeemable_mint)]
    pub redeemable_mint: Box<Account<'info, Mint>>,
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExchangeUsdcForRedeemable<'info> {
    // User Accounts
    pub user_authority: Signer<'info>,
    // TODO replace these with the ATA constraints when possible
    #[account(mut,
        constraint = user_usdc.owner == user_authority.key(),
        constraint = user_usdc.mint == usdc_mint.key())]
    pub user_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        seeds = [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"user_redeemable"],
        bump)]
    pub user_redeemable: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account,
        has_one = usdc_mint)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    pub usdc_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"redeemable_mint"],
        bump = ido_account.bumps.redeemable_mint)]
    pub redeemable_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"pool_usdc"],
        bump = ido_account.bumps.pool_usdc)]
    pub pool_usdc: Box<Account<'info, TokenAccount>>,
    // Programs and Sysvars
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct InitEscrowUsdc<'info> {
    // User Accounts
    #[account(mut)]
    pub user_authority: Signer<'info>,
    #[account(init,
        token::mint = usdc_mint,
        token::authority = ido_account,
        seeds =  [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"escrow_usdc"],
        bump,
        payer = user_authority
    )]
    pub escrow_usdc: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account,
        has_one = usdc_mint)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    pub usdc_mint: Box<Account<'info, Mint>>,
    // Programs and Sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExchangeRedeemableForUsdc<'info> {
    // User Accounts
    pub user_authority: Signer<'info>,
    #[account(mut,
        seeds = [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"escrow_usdc"],
        bump)]
    pub escrow_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        seeds = [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"user_redeemable"],
        bump)]
    pub user_redeemable: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account,
        has_one = usdc_mint)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    pub usdc_mint: Box<Account<'info, Mint>>,
    pub watermelon_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"redeemable_mint"],
        bump = ido_account.bumps.redeemable_mint)]
    pub redeemable_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"pool_usdc"],
        bump = ido_account.bumps.pool_usdc)]
    pub pool_usdc: Box<Account<'info, TokenAccount>>,
    // Programs and Sysvars
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ExchangeRedeemableForWatermelon<'info> {
    // User does not have to sign, this allows anyone to redeem on their behalf
    // and prevents forgotten / leftover redeemable tokens in the IDO pool.
    pub payer: Signer<'info>,
    // User Accounts
    #[account(mut)] // Sol rent from empty redeemable account is refunded to the user
    pub user_authority: UncheckedAccount<'info>,
    // TODO replace with ATA constraints
    #[account(mut,
        constraint = user_watermelon.owner == user_authority.key(),
        constraint = user_watermelon.mint == watermelon_mint.key())]
    pub user_watermelon: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        seeds = [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"user_redeemable"],
        bump)]
    pub user_redeemable: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account,
        has_one = watermelon_mint)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    pub watermelon_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"redeemable_mint"],
        bump = ido_account.bumps.redeemable_mint)]
    pub redeemable_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"pool_watermelon"],
        bump = ido_account.bumps.pool_watermelon)]
    pub pool_watermelon: Box<Account<'info, TokenAccount>>,
    // Programs and Sysvars
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawPoolUsdc<'info> {
    // IDO Authority Accounts
    pub ido_authority: Signer<'info>,
    // Doesn't need to be an ATA because it might be a DAO account
    #[account(mut,
        constraint = ido_authority_usdc.owner == ido_authority.key(),
        constraint = ido_authority_usdc.mint == usdc_mint.key())]
    pub ido_authority_usdc: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account,
        has_one = ido_authority,
        has_one = usdc_mint,
        has_one = watermelon_mint)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    pub usdc_mint: Box<Account<'info, Mint>>,
    pub watermelon_mint: Box<Account<'info, Mint>>,
    #[account(mut,
        seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace(), b"pool_usdc"],
        bump = ido_account.bumps.pool_usdc)]
    pub pool_usdc: Box<Account<'info, TokenAccount>>,
    // Program and Sysvars
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawFromEscrow<'info> {
    // User does not have to sign, this allows anyone to redeem on their behalf
    // and prevents forgotten / leftover USDC in the IDO pool.
    pub payer: Signer<'info>,
    // User Accounts
    #[account(mut)]
    pub user_authority: UncheckedAccount<'info>,
    #[account(mut,
        constraint = user_usdc.owner == user_authority.key(),
        constraint = user_usdc.mint == usdc_mint.key())]
    pub user_usdc: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        seeds = [user_authority.key().as_ref(),
            ido_account.ido_name.as_ref().trim_ascii_whitespace(),
            b"escrow_usdc"],
        bump)]
    pub escrow_usdc: Box<Account<'info, TokenAccount>>,
    // IDO Accounts
    #[account(seeds = [ido_account.ido_name.as_ref().trim_ascii_whitespace()],
        bump = ido_account.bumps.ido_account,
        has_one = usdc_mint)]
    pub ido_account: Box<Account<'info, IdoAccount>>,
    pub usdc_mint: Box<Account<'info, Mint>>,
    // Programs and Sysvars
    pub token_program: Program<'info, Token>,
}

#[account]
#[derive(InitSpace)]
pub struct IdoAccount {
    pub ido_name: [u8; 10], // Setting an arbitrary max of ten characters in the ido name. // 10
    pub bumps: PoolBumps,   // 4
    pub ido_authority: Pubkey, // 32

    pub usdc_mint: Pubkey,       // 32
    pub redeemable_mint: Pubkey, // 32
    pub watermelon_mint: Pubkey, // 32
    pub pool_usdc: Pubkey,       // 32
    pub pool_watermelon: Pubkey, // 32

    pub num_ido_tokens: u64, // 8
    pub ido_times: IdoTimes, // 32
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Default, Clone, Copy)]
pub struct IdoTimes {
    pub start_ido: i64,    // 8
    pub end_deposits: i64, // 8
    pub end_ido: i64,      // 8
    pub end_escrow: i64,   // 8
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Default, Clone)]
pub struct PoolBumps {
    pub ido_account: u8,     // 1
    pub redeemable_mint: u8, // 1
    pub pool_watermelon: u8, // 1
    pub pool_usdc: u8,       // 1
}

#[error_code]
pub enum ErrorCode {
    #[msg("IDO must start in the future")]
    IdoFuture,
    #[msg("IDO times are non-sequential")]
    SeqTimes,
    #[msg("IDO has not started")]
    StartIdoTime,
    #[msg("Deposits period has ended")]
    EndDepositsTime,
    #[msg("IDO has ended")]
    EndIdoTime,
    #[msg("IDO has not finished yet")]
    IdoNotOver,
    #[msg("Escrow period has not finished yet")]
    EscrowNotOver,
    #[msg("Insufficient USDC")]
    LowUsdc,
    #[msg("Insufficient redeemable tokens")]
    LowRedeemable,
    #[msg("USDC total and redeemable total don't match")]
    UsdcNotEqRedeem,
    #[msg("Given nonce is invalid")]
    InvalidNonce,
}

// Access control modifiers.

// Asserts the IDO starts in the future.
fn validate_ido_times(ido_times: IdoTimes) -> Result<()> {
    let clock = Clock::get()?;
    if ido_times.start_ido <= clock.unix_timestamp {
        return err!(ErrorCode::IdoFuture);
    }
    if !(ido_times.start_ido < ido_times.end_deposits
        && ido_times.end_deposits < ido_times.end_ido
        && ido_times.end_ido < ido_times.end_escrow)
    {
        return err!(ErrorCode::SeqTimes);
    }
    Ok(())
}

// Asserts the IDO is still accepting deposits.
fn unrestricted_phase(ido_account: &IdoAccount) -> Result<()> {
    let clock = Clock::get()?;
    if clock.unix_timestamp <= ido_account.ido_times.start_ido {
        return err!(ErrorCode::StartIdoTime);
    } else if ido_account.ido_times.end_deposits <= clock.unix_timestamp {
        return err!(ErrorCode::EndDepositsTime);
    }
    Ok(())
}

// Asserts the IDO has started but not yet finished.
fn withdraw_phase(ido_account: &IdoAccount) -> Result<()> {
    let clock = Clock::get()?;
    if clock.unix_timestamp <= ido_account.ido_times.start_ido {
        return err!(ErrorCode::StartIdoTime);
    } else if ido_account.ido_times.end_ido <= clock.unix_timestamp {
        return err!(ErrorCode::EndIdoTime);
    }
    Ok(())
}

// Asserts the IDO sale period has ended.
fn ido_over(ido_account: &IdoAccount) -> Result<()> {
    let clock = Clock::get()?;
    if clock.unix_timestamp <= ido_account.ido_times.end_ido {
        return err!(ErrorCode::IdoNotOver);
    }
    Ok(())
}

fn escrow_over(ido_account: &IdoAccount) -> Result<()> {
    let clock = Clock::get()?;
    if clock.unix_timestamp <= ido_account.ido_times.end_escrow {
        return err!(ErrorCode::EscrowNotOver);
    }
    Ok(())
}

/// Trait to allow trimming ascii whitespace from a &[u8].
pub trait TrimAsciiWhitespace {
    /// Trim ascii whitespace (based on `is_ascii_whitespace()`) from the
    /// start and end of a slice.
    fn trim_ascii_whitespace(&self) -> &[u8];
}

impl<T: Deref<Target = [u8]>> TrimAsciiWhitespace for T {
    fn trim_ascii_whitespace(&self) -> &[u8] {
        let from = match self.iter().position(|x| !x.is_ascii_whitespace()) {
            Some(i) => i,
            None => return &self[0..0],
        };
        let to = self.iter().rposition(|x| !x.is_ascii_whitespace()).unwrap();
        &self[from..=to]
    }
}
