use anchor_lang::prelude::*;
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
use anchor_lang::FixedVec;

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
        let cpi_ctx =
            CpiContext::<_, 2>::new(ctx.accounts.external_program.address(), cpi_accounts);
        external_cpi::cpi::set_value(cpi_ctx, value)?;
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
        let cpi_ctx =
            CpiContext::<_, 3>::new(ctx.accounts.external_program.address(), cpi_accounts);
        external_cpi::cpi::composite(cpi_ctx, count)?;
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
        let cpi_ctx =
            CpiContext::<_, 2>::new(ctx.accounts.external_program.address(), cpi_accounts);
        let args = external_cpi::MyArgs {
            amount,
            tag,
            owner: *ctx.accounts.authority.address(),
        };
        // `MyArgs` is fixed-size, so the IDL bounds it and both targets
        // share one signature.
        external_cpi::cpi::defined_args(cpi_ctx, args)?;
        Ok(())
    }

    #[discrim = 3]
    pub fn proxy_alt_bump(ctx: &mut Context<ProxyAlt>, delta: u8) -> Result<()> {
        let cpi_accounts = alt_cpi::cpi::accounts::Bump {
            data: ctx.accounts.data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::<_, 2>::new(ctx.accounts.alt_program.address(), cpi_accounts);
        alt_cpi::cpi::bump(cpi_ctx, delta)?;
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
        let cpi_ctx = CpiContext::<_, 2>::new(ctx.accounts.hash_program.address(), cpi_accounts);
        hash_cpi::cpi::apply(cpi_ctx, delta, flag, marker)?;
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

/// `MemoArgs` holds `bytes`, so the IDL gives no bound and SBF callers
/// still pass instruction data storage.
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub fn memo_args_take_data_buffer<'a>(
    ctx: CpiContext<'a, external::cpi::accounts::Memo<'a>, 1>,
    args: external::MemoArgs<'_>,
    data: &mut FixedVec<u8, 64>,
) -> Result<()> {
    external::cpi::memo(ctx, args, data)
}

#[cfg(feature = "cpi")]
pub fn cpi_account_type_is_generated<'a>(
    accounts: external::cpi::accounts::Composite<'a>,
) -> external::cpi::accounts::Composite<'a> {
    accounts
}
