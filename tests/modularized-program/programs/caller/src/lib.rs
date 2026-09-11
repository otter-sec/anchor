//! CPIs into the `modularized` program, exercising the generated
//! `cpi::accounts` module for `Accounts` structs defined in nested modules.

use anchor_lang::prelude::*;
use modularized::program::Modularized;

declare_id!("Modu1arizedCa11er11111111111111111111111111");

#[program]
pub mod caller {
    use super::*;

    pub fn proxy_init(ctx: Context<ProxyInit>, data: u64) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.modularized_program.key(),
            modularized::cpi::accounts::Init {
                counter: ctx.accounts.counter.to_account_info(),
                payer: ctx.accounts.payer.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
            },
        );
        modularized::cpi::init(cpi_ctx, data)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProxyInit<'info> {
    /// CHECK: Initialized by the modularized program via CPI.
    #[account(mut)]
    pub counter: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub modularized_program: Program<'info, Modularized>,
}
