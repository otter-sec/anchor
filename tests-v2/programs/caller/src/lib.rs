use anchor_lang_v2::prelude::*;

declare_id!("8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR");

/// Mirror of callee's DataAccount for cross-program typed loading.
///
/// Implements `Owner` to return the **callee** program ID so that
/// `Account<CalleeData>` passes the Slab owner check when the account
/// is owned by the callee. Layout and discriminator must match exactly.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CalleeData {
    pub value: u64,
    pub authority: Address,
}

impl Owner for CalleeData {
    const OWNER: Address = callee::ID;
}

impl Discriminator for CalleeData {
    const DISCRIMINATOR: &'static [u8] = callee::DataAccount::DISCRIMINATOR;
}

#[program]
pub mod caller {
    use super::*;

    pub fn proxy_set_data(ctx: &mut Context<ProxySetData>, value: u64) -> Result<()> {
        let cpi_accounts = callee::cpi::accounts::SetData {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        callee::cpi::set_data(cpi_ctx, value);
        Ok(())
    }

    /// CPIs into the no-args `noop` handler — exercises the empty-args
    /// branch of the auto-generated cpi wrapper.
    pub fn proxy_noop(ctx: &mut Context<ProxySetData>) -> Result<()> {
        let cpi_accounts = callee::cpi::accounts::SetData {
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        callee::cpi::noop(cpi_ctx);
        Ok(())
    }

    /// CPIs into the zero-account `empty` handler. Constructs the
    /// auto-generated `cpi::accounts::Empty` via its `new()` ctor — the
    /// only viable route since the lifetime-anchoring `_phantom` field
    /// is hidden.
    pub fn proxy_empty(ctx: &mut Context<ProxyEmpty>) -> Result<()> {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.callee_program.address(),
            callee::cpi::accounts::Empty::new(),
        );
        callee::cpi::empty(cpi_ctx);
        Ok(())
    }

    /// CPIs into `touch`, whose `Touch` accounts struct exercises every
    /// writable/signer combination so the auto-generated impl emits all
    /// four `InstructionAccount::*` ctors.
    pub fn proxy_touch(ctx: &mut Context<ProxyTouch>, delta: u64) -> Result<()> {
        let cpi_accounts = callee::cpi::accounts::Touch {
            payer: ctx.accounts.payer.cpi_handle_mut(),
            data: ctx.accounts.callee_data.cpi_handle_mut(),
            authority: ctx.accounts.authority.cpi_handle(),
            spectator: ctx.accounts.spectator.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.callee_program.address(), cpi_accounts);
        callee::cpi::touch(cpi_ctx, delta);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ProxySetData {
    /// Loaded as a Slab-backed Account — Slab sets borrow_state = 0,
    /// which would fail pinocchio's checked invoke. Our CpiContext uses
    /// invoke_signed_unchecked to bypass this.
    #[account(mut)]
    pub callee_data: Account<CalleeData>,
    pub authority: Signer,
    pub callee_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ProxyEmpty {
    pub callee_program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ProxyTouch {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub callee_data: Account<CalleeData>,
    pub authority: Signer,
    pub spectator: UncheckedAccount,
    pub callee_program: UncheckedAccount,
}
