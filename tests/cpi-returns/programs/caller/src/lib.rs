use anchor_lang::prelude::*;
use callee::cpi::accounts::CpiReturn;
use callee::program::Callee;
use callee::{self, CpiReturnAccount};
use malicious::cpi::accounts::SpoofReturn;
use malicious::program::Malicious;

declare_id!("HmbTLCmaGvZhKnn1Zfa1JVnp7vkMV4DYVxPLWBVoN65L");

#[program]
pub mod caller {
    use super::*;

    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct Struct {
        pub a: u64,
        pub b: u64,
    }

    pub fn cpi_call_return_u64(ctx: Context<CpiReturnContext>) -> Result<()> {
        let cpi_program_id = ctx.accounts.cpi_return_program.key();
        let cpi_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        let result = callee::cpi::return_u64(cpi_ctx)?;
        let solana_return = result.get();
        anchor_lang::solana_program::log::sol_log_data(&[&borsh::to_vec(&solana_return).unwrap()]);
        Ok(())
    }

    pub fn cpi_call_return_u64_same_program_spoofed(ctx: Context<CpiReturnContext>) -> Result<()> {
        let cpi_program_id = ctx.accounts.cpi_return_program.key();
        let cpi_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        let result = callee::cpi::return_u64(cpi_ctx)?;

        let spoof_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let spoof_ctx = CpiContext::new(cpi_program_id, spoof_accounts);
        callee::cpi::return_u64_spoofed(spoof_ctx)?;

        let solana_return = result.get();
        anchor_lang::solana_program::log::sol_log_data(&[&borsh::to_vec(&solana_return).unwrap()]);
        Ok(())
    }

    pub fn cpi_call_return_struct(ctx: Context<CpiReturnContext>) -> Result<()> {
        let cpi_program_id = ctx.accounts.cpi_return_program.key();
        let cpi_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        let result = callee::cpi::return_struct(cpi_ctx)?;
        let solana_return = result.get();
        anchor_lang::solana_program::log::sol_log_data(&[&borsh::to_vec(&solana_return).unwrap()]);
        Ok(())
    }

    pub fn cpi_call_return_vec(ctx: Context<CpiReturnContext>) -> Result<()> {
        let cpi_program_id = ctx.accounts.cpi_return_program.key();
        let cpi_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        let result = callee::cpi::return_vec(cpi_ctx)?;
        let solana_return = result.get();
        anchor_lang::solana_program::log::sol_log_data(&[&borsh::to_vec(&solana_return).unwrap()]);
        Ok(())
    }

    pub fn return_u64(_ctx: Context<ReturnContext>) -> Result<u64> {
        Ok(99)
    }

    pub fn return_struct(_ctx: Context<ReturnContext>) -> Result<Struct> {
        Ok(Struct { a: 1, b: 2 })
    }

    pub fn return_vec(_ctx: Context<ReturnContext>) -> Result<Vec<u64>> {
        Ok(vec![1, 2, 3])
    }

    /// PoC: Demonstrates that get_unchecked() reads spoofed return data.
    /// This replicates the OLD (vulnerable) behavior of get().
    ///
    /// 1. CPI to callee::return_u64 -> callee sets return data = 10
    /// 2. CPI to malicious::spoof_return_data -> overwrites return data with 999
    /// 3. get_unchecked() reads 999 instead of 10 (SPOOFED!)
    pub fn cpi_call_return_u64_spoofed(ctx: Context<SpoofedReturnContext>) -> Result<()> {
        // Step 1: CPI to callee, which returns u64 = 10
        let cpi_program_id = ctx.accounts.cpi_return_program.key();
        let cpi_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        let result = callee::cpi::return_u64(cpi_ctx)?;

        // Step 2: CPI to malicious program, which calls set_return_data(999)
        let malicious_program_id = ctx.accounts.malicious_program.key();
        let spoof_accounts = SpoofReturn {
            authority: ctx.accounts.authority.to_account_info(),
        };
        let spoof_ctx = CpiContext::new(malicious_program_id, spoof_accounts);
        malicious::cpi::spoof_return_data(spoof_ctx)?;

        // Step 3: Use get_unchecked() (old vulnerable behavior) to read the
        // spoofed return data without program_id validation.
        let spoofed_value = result.get_unchecked();

        // Log the spoofed value so the test can verify it
        anchor_lang::solana_program::log::sol_log_data(&[&borsh::to_vec(&spoofed_value).unwrap()]);

        Ok(())
    }

    /// Demonstrates that get() (with fix) preserves the original return data
    /// even if a later CPI from a different program overwrites the global
    /// return-data slot.
    ///
    /// Same flow as above, but uses get() instead of get_unchecked().
    /// Because get() now reads the snapshot captured immediately after the
    /// callee CPI returns, the later malicious overwrite is ignored.
    pub fn cpi_call_return_u64_spoofed_preserved(ctx: Context<SpoofedReturnContext>) -> Result<()> {
        // Step 1: CPI to callee, which returns u64 = 10
        let cpi_program_id = ctx.accounts.cpi_return_program.key();
        let cpi_accounts = CpiReturn {
            account: ctx.accounts.cpi_return.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program_id, cpi_accounts);
        let result = callee::cpi::return_u64(cpi_ctx)?;

        // Step 2: CPI to malicious program, which calls set_return_data(999)
        let malicious_program_id = ctx.accounts.malicious_program.key();
        let spoof_accounts = SpoofReturn {
            authority: ctx.accounts.authority.to_account_info(),
        };
        let spoof_ctx = CpiContext::new(malicious_program_id, spoof_accounts);
        malicious::cpi::spoof_return_data(spoof_ctx)?;

        // Step 3: Use get() (FIXED) — this reads the invoke-time snapshot, not
        // the current global return-data slot.
        let value = result.get();
        anchor_lang::solana_program::log::sol_log_data(&[&borsh::to_vec(&value).unwrap()]);

        Ok(())
    }
}

#[derive(Accounts)]
pub struct CpiReturnContext<'info> {
    #[account(mut)]
    pub cpi_return: Account<'info, CpiReturnAccount>,
    pub cpi_return_program: Program<'info, Callee>,
}

#[derive(Accounts)]
pub struct SpoofedReturnContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub cpi_return: Account<'info, CpiReturnAccount>,
    pub cpi_return_program: Program<'info, Callee>,
    pub malicious_program: Program<'info, Malicious>,
}

#[derive(Accounts)]
pub struct ReturnContext {}
