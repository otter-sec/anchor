use anchor_lang_v2::prelude::*;

declare_id!("Dec1areProgram11111111111111111111111111111");

declare_program!(optional_callee);

#[program]
pub mod declare_program_optional {
    use super::*;

    #[discrim = 0]
    pub fn proxy_record(ctx: &mut Context<ProxyRecord>, value: u64) -> Result<()> {
        let cpi_accounts = optional_callee::cpi::accounts::Record {
            data: ctx.accounts.data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
            maybe_marker: ctx
                .accounts
                .maybe_marker
                .as_ref()
                .map(|account| account.cpi_handle()),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.optional_program.address(), cpi_accounts);
        optional_callee::cpi::record(cpi_ctx, value);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProxyRecord {
    #[account(mut)]
    pub data: UncheckedAccount,
    pub authority: Signer,
    pub maybe_marker: Option<UncheckedAccount>,
    pub optional_program: UncheckedAccount,
}
