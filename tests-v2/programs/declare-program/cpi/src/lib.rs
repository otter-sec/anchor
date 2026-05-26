use anchor_lang_v2::prelude::*;

declare_id!("Dec1areProgram11111111111111111111111111111");

declare_program!(external);
declare_program!(external_cpi);
declare_program!(alt_cpi);
declare_program!(hash_cpi);

#[program]
pub mod declare_program {
    use super::*;

    #[discrim = 0]
    pub fn proxy_set_value(ctx: &mut Context<ProxyExternal>, value: u64) -> Result<()> {
        let cpi_accounts = external_cpi::cpi::accounts::SetValue {
            data: ctx.accounts.data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.external_program.address(), cpi_accounts);
        external_cpi::cpi::set_value(cpi_ctx, value);
        Ok(())
    }

    #[discrim = 1]
    pub fn proxy_composite(ctx: &mut Context<ProxyExternalComposite>, count: u16) -> Result<()> {
        let cpi_accounts = external_cpi::cpi::accounts::Composite {
            inner: external_cpi::__cpi_accounts_inner::Inner {
                data: ctx.accounts.data.cpi_handle_mut(),
                authority: ctx.accounts.authority.cpi_handle(),
            },
            payer: ctx.accounts.payer.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.external_program.address(), cpi_accounts);
        external_cpi::cpi::composite(cpi_ctx, count);
        Ok(())
    }

    #[discrim = 2]
    pub fn proxy_defined_args(
        ctx: &mut Context<ProxyExternal>,
        amount: u64,
        tag: [u8; 3],
    ) -> Result<()> {
        let cpi_accounts = external_cpi::cpi::accounts::DefinedArgs {
            data: ctx.accounts.data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.external_program.address(), cpi_accounts);
        external_cpi::cpi::defined_args(
            cpi_ctx,
            external_cpi::MyArgs {
                amount,
                tag,
                owner: *ctx.accounts.authority.address(),
            },
        );
        Ok(())
    }

    #[discrim = 3]
    pub fn proxy_alt_bump(ctx: &mut Context<ProxyAlt>, delta: u8) -> Result<()> {
        let cpi_accounts = alt_cpi::cpi::accounts::Bump {
            data: ctx.accounts.data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.alt_program.address(), cpi_accounts);
        alt_cpi::cpi::bump(cpi_ctx, delta);
        Ok(())
    }

    #[discrim = 4]
    pub fn proxy_hash_apply(
        ctx: &mut Context<ProxyHash>,
        delta: i64,
        flag: bool,
        marker: [u8; 4],
    ) -> Result<()> {
        let cpi_accounts = hash_cpi::cpi::accounts::Apply {
            data: ctx.accounts.data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.hash_program.address(), cpi_accounts);
        hash_cpi::cpi::apply(cpi_ctx, delta, flag, marker);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProxyExternal {
    #[account(mut)]
    pub data: UncheckedAccount,
    pub authority: Signer,
    pub external_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ProxyExternalComposite {
    #[account(mut)]
    pub data: UncheckedAccount,
    pub authority: Signer,
    #[account(mut)]
    pub payer: Signer,
    pub external_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ProxyAlt {
    #[account(mut)]
    pub data: UncheckedAccount,
    pub authority: Signer,
    pub alt_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ProxyHash {
    #[account(mut)]
    pub data: UncheckedAccount,
    pub authority: Signer,
    pub hash_program: UncheckedAccount,
}

#[cfg(feature = "cpi")]
pub fn cpi_account_type_is_generated<'a>(
    accounts: external::cpi::accounts::Composite<'a>,
) -> external::cpi::accounts::Composite<'a> {
    accounts
}
