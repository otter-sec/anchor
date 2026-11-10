//! Calls `caller`, which calls `callee`, so a single SBF link holds two
//! `cpi` dependencies. See `tests/nested_cpi.rs`.

use anchor_lang::prelude::*;

declare_id!("4cgwTgJ3uQwGpgs5C8DYpsoFT9UwYnjucSyVg97Yza61");

#[program]
pub mod nested_caller {
    use super::*;

    /// CPIs into `caller::proxy_empty`, which CPIs into `callee::empty`.
    pub fn proxy_proxy_empty(ctx: &mut Context<ProxyProxyEmpty>) -> Result<()> {
        let cpi_accounts = caller::cpi::accounts::ProxyEmpty {
            callee_program: ctx.accounts.callee_program.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.caller_program.address(), cpi_accounts);
        caller::cpi::proxy_empty(cpi_ctx)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProxyProxyEmpty {
    pub caller_program: UncheckedAccount,
    pub callee_program: UncheckedAccount,
}
