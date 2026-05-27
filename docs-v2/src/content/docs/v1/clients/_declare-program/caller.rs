use anchor_lang::prelude::*;

declare_id!("GENmb1D59wqCKRwujq4PJ8461EccQ5srLHrXyXp4HMTH");

declare_program!(example);
use example::{
    accounts::Counter, // Account types
    cpi::{             // Cross program invocation helpers
        self,
        accounts::{Increment, Initialize},
    },
    program::Example,  // Program type
};

#[program]
pub mod example_cpi {

    use super::*;

    pub fn initialize_cpi(ctx: Context<InitializeCpi>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.example_program.key(),
            Initialize {
                payer: ctx.accounts.payer.to_account_info(),
                counter: ctx.accounts.counter.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
        );

        cpi::initialize(cpi_ctx)?;
        Ok(())
    }

    pub fn increment_cpi(ctx: Context<IncrementCpi>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.example_program.key(),
            Increment {
                counter: ctx.accounts.counter.to_account_info(),
            },
        );

        cpi::increment(cpi_ctx)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeCpi<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut)]
    pub counter: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub example_program: Program<'info, Example>,
}

#[derive(Accounts)]
pub struct IncrementCpi<'info> {
    // Counter type from accounts module
    #[account(mut)]
    pub counter: Account<'info, Counter>,

    // Example type from program module
    pub example_program: Program<'info, Example>,
}
