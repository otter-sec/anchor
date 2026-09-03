use {anchor_lang::prelude::*, anchor_spl::token_2022_extensions as token_2022_ext};

declare_id!("CvCYVXhFDScZ8CNRtm6mSU8AkZrN5tk3NcFF8Q33M45z");

#[program]
pub mod token_2022_ext_transfer_fee {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>, config_authority: Address) -> Result<()> {
        let accs = token_2022_ext::TransferFeeInitialize {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::transfer_fee_initialize(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            Some(&config_authority),
            Some(&config_authority),
            111,
            42,
        )?;
        Ok(())
    }

    #[discrim = 1]
    pub fn set(ctx: &mut Context<Set>) -> Result<()> {
        let accs = token_2022_ext::TransferFeeSetTransferFee {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::transfer_fee_set(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            222,
            84,
        )?;
        Ok(())
    }

    #[discrim = 2]
    pub fn transfer_checked_with_fee(ctx: &mut Context<TransferCheckedWithFee>) -> Result<()> {
        let accs = token_2022_ext::TransferCheckedWithFee {
            source: ctx.accounts.source.cpi_handle_mut(),
            mint: ctx.accounts.mint.cpi_handle(),
            destination: ctx.accounts.destination.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::transfer_checked_with_fee(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            500,
            6,
            6,
        )?;
        Ok(())
    }

    #[discrim = 3]
    pub fn harvest_withheld_tokens_to_mint(ctx: &mut Context<Harvest>) -> Result<()> {
        let accs = token_2022_ext::HarvestWithheldTokensToMint {
            mint: ctx.accounts.mint.cpi_handle_mut(),
        };
        token_2022_ext::harvest_withheld_tokens_to_mint(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            vec![ctx.accounts.source.cpi_handle_mut()],
        )?;
        Ok(())
    }

    #[discrim = 4]
    pub fn withdraw_withheld_tokens_from_mint(ctx: &mut Context<WithdrawFromMint>) -> Result<()> {
        let accs = token_2022_ext::WithdrawWithheldTokensFromMint {
            mint: ctx.accounts.mint.cpi_handle_mut(),
            destination: ctx.accounts.destination.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::withdraw_withheld_tokens_from_mint(CpiContext::new(
            ctx.accounts.token_program.address(),
            accs,
        ))?;
        Ok(())
    }

    #[discrim = 5]
    pub fn withdraw_withheld_tokens_from_accounts(
        ctx: &mut Context<WithdrawFromAccounts>,
    ) -> Result<()> {
        let accs = token_2022_ext::WithdrawWithheldTokensFromAccounts {
            mint: ctx.accounts.mint.cpi_handle(),
            destination: ctx.accounts.destination.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        token_2022_ext::withdraw_withheld_tokens_from_accounts(
            CpiContext::new(ctx.accounts.token_program.address(), accs),
            vec![ctx.accounts.source.cpi_handle_mut()],
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(config_authority: Address)]
pub struct Initialize {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Set {
    #[account(mut)]
    pub mint: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct TransferCheckedWithFee {
    #[account(mut)]
    pub source: UncheckedAccount,
    pub mint: UncheckedAccount,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct Harvest {
    #[account(mut)]
    pub mint: UncheckedAccount,
    #[account(mut)]
    pub source: UncheckedAccount,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct WithdrawFromMint {
    #[account(mut)]
    pub mint: UncheckedAccount,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub authority: Signer,
    pub token_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct WithdrawFromAccounts {
    pub mint: UncheckedAccount,
    #[account(mut)]
    pub destination: UncheckedAccount,
    pub authority: Signer,
    #[account(mut)]
    pub source: UncheckedAccount,
    pub token_program: UncheckedAccount,
}
