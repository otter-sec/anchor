use anchor_lang_v2::prelude::*;

declare_id!("Dec1areProgram11111111111111111111111111111");

declare_program!(return_callee);

pub const PROXY_SEED: &[u8] = b"return-proxy";

#[account]
#[repr(C)]
pub struct ProxyResult {
    pub authority: Address,
    pub last_return: u64,
    pub last_observed: u64,
    pub last_payload_amount: u64,
    pub last_payload_sample_sum: u64,
    pub last_payload_label_len: u32,
    pub calls: u16,
    pub last_payload_has_authority: u8,
    pub bump: u8,
}

#[program]
pub mod declare_program_returns {
    use super::*;

    #[discrim = 0]
    pub fn initialize_result(ctx: &mut Context<InitializeResult>) -> Result<()> {
        ctx.accounts.result.authority = *ctx.accounts.authority.address();
        ctx.accounts.result.last_return = 0;
        ctx.accounts.result.last_observed = 0;
        ctx.accounts.result.last_payload_amount = 0;
        ctx.accounts.result.last_payload_sample_sum = 0;
        ctx.accounts.result.last_payload_label_len = 0;
        ctx.accounts.result.last_payload_has_authority = 0;
        ctx.accounts.result.calls = 0;
        ctx.accounts.result.bump = ctx.bumps.result;
        Ok(())
    }

    #[discrim = 1]
    pub fn proxy_calculate(ctx: &mut Context<ProxyCalculate>, base: u64, bonus: u16) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::Calculate {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::calculate(cpi_ctx, base, bonus)?.get();
        ctx.accounts.result.calls = ctx.accounts.result.calls.saturating_add(1);
        ctx.accounts.result.last_return = returned;
        ctx.accounts.result.last_observed =
            returned.saturating_add(ctx.accounts.result.calls as u64);
        Ok(())
    }

    #[discrim = 2]
    pub fn proxy_describe(ctx: &mut Context<ProxyCalculate>, base: u64, bonus: u16) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::Describe {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::describe(cpi_ctx, base, bonus)?.get();
        ctx.accounts.result.calls = ctx.accounts.result.calls.saturating_add(1);
        ctx.accounts.result.last_payload_amount = returned.amount;
        ctx.accounts.result.last_payload_label_len = returned.label.len() as u32;
        ctx.accounts.result.last_payload_sample_sum =
            returned.samples.iter().map(|value| *value as u64).sum();
        ctx.accounts.result.last_payload_has_authority = returned.maybe_authority.is_some() as u8;
        Ok(())
    }

    #[discrim = 3]
    pub fn proxy_no_return(ctx: &mut Context<ProxyCalculate>, base: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::NoReturnButIdlSays {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::no_return_but_idl_says(cpi_ctx, base)?.get();
        ctx.accounts.result.last_return = returned;
        Ok(())
    }

    #[discrim = 4]
    pub fn proxy_short_return(ctx: &mut Context<ProxyCalculate>, base: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::ShortReturn {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::short_return(cpi_ctx, base)?.get();
        ctx.accounts.result.last_return = returned;
        Ok(())
    }

    #[discrim = 5]
    pub fn proxy_spoofed_return(ctx: &mut Context<ProxySpoofedReturn>, base: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::SpoofedReturn {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
            spoof_program: ctx.accounts.spoof_program.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::spoofed_return(cpi_ctx, base)?.get();
        ctx.accounts.result.last_return = returned;
        Ok(())
    }

    #[discrim = 6]
    pub fn proxy_malformed_payload(ctx: &mut Context<ProxyCalculate>, base: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::MalformedPayload {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::malformed_payload(cpi_ctx, base)?.get();
        ctx.accounts.result.last_payload_amount = returned.amount;
        Ok(())
    }

    #[discrim = 7]
    pub fn proxy_describe_empty(
        ctx: &mut Context<ProxyCalculate>,
        base: u64,
        bonus: u16,
    ) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.result.authority,
            *ctx.accounts.authority.address()
        );
        let cpi_accounts = return_callee::cpi::accounts::DescribeEmpty {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        let returned = return_callee::cpi::describe_empty(cpi_ctx, base, bonus)?.get();
        ctx.accounts.result.calls = ctx.accounts.result.calls.saturating_add(1);
        ctx.accounts.result.last_payload_amount = returned.amount;
        ctx.accounts.result.last_payload_label_len = returned.label.len() as u32;
        ctx.accounts.result.last_payload_sample_sum =
            returned.samples.iter().map(|value| *value as u64).sum();
        ctx.accounts.result.last_payload_has_authority = returned.maybe_authority.is_some() as u8;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeResult {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + core::mem::size_of::<ProxyResult>(),
        seeds = [PROXY_SEED, authority.address().as_ref()],
        bump,
    )]
    pub result: Account<ProxyResult>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct ProxyCalculate {
    #[account(mut, seeds = [PROXY_SEED, authority.address().as_ref()], bump)]
    pub result: Account<ProxyResult>,
    #[account(mut)]
    pub callee_data: UncheckedAccount,
    pub authority: Signer,
    pub callee_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ProxySpoofedReturn {
    #[account(mut, seeds = [PROXY_SEED, authority.address().as_ref()], bump)]
    pub result: Account<ProxyResult>,
    #[account(mut)]
    pub callee_data: UncheckedAccount,
    pub authority: Signer,
    pub callee_program: UncheckedAccount,
    pub spoof_program: UncheckedAccount,
}
