use anchor_lang::prelude::*;

declare_id!("4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi");

#[program]
pub mod callee {
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.value = 0;
        ctx.accounts.data.authority = *ctx.accounts.authority.address();
        Ok(())
    }

    pub fn set_data(ctx: &mut Context<SetData>, value: u64) -> Result<()> {
        ctx.accounts.data.value = value;
        Ok(())
    }

    /// No-extra-args handler reusing `SetData`. Drives the
    /// `extra_arg_names.is_empty()` branch in the cpi-wrapper codegen and
    /// the dedupe of `cpi::accounts` re-exports — `noop` and `set_data`
    /// share the same Accounts struct, so only one `pub use` lands.
    pub fn noop(_ctx: &mut Context<SetData>) -> Result<()> {
        Ok(())
    }

    /// Zero-account handler — drives the empty-fields branch of the cpi
    /// accounts codegen, where `'a` has no `CpiHandle` field to anchor it
    /// and would otherwise fail E0392.
    pub fn empty(_ctx: &mut Context<Empty>) -> Result<()> {
        Ok(())
    }

    /// Drives all four `InstructionAccount::{writable_signer, writable,
    /// readonly_signer, readonly}` ctor branches of the auto-generated
    /// `ToCpiAccounts` impl.
    pub fn touch(ctx: &mut Context<Touch>, delta: u64) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.data.authority,
            *ctx.accounts.authority.address(),
            ErrorCode::ConstraintAddress
        );
        ctx.accounts.data.value = ctx.accounts.data.value.saturating_add(delta);
        Ok(())
    }

    /// Zero-byte return type — Finding #81: empty structs serialize to `[]`,
    /// which the runtime exposes as `None` from `get_return_data`. CPI
    /// callers must still be able to `Return<Ack>::get()`.
    pub fn acknowledge(_ctx: &mut Context<Empty>) -> Result<Ack> {
        Ok(Ack {})
    }
}

/// Empty return payload with a valid zero-byte wincode schema.
#[derive(Clone, Copy, wincode::SchemaRead, wincode::SchemaWrite)]
pub struct Ack {}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"data"],
        bump,
    )]
    pub data: Account<DataAccount>,
    pub authority: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct SetData {
    #[account(mut, seeds = [b"data"], bump)]
    pub data: Account<DataAccount>,
    #[account(address = data.authority)]
    pub authority: Signer,
}

/// Empty Accounts struct — exercises the `'a`-anchoring `PhantomData`
/// fallback in the auto-generated `__cpi_accounts_empty` module.
#[derive(Accounts)]
pub struct Empty {}

/// Mixes one of each writable/signer combination so the cpi-accounts
/// codegen has to emit each `InstructionAccount` ctor variant.
#[derive(Accounts)]
pub struct Touch {
    /// `writable_signer`
    #[account(mut)]
    pub payer: Signer,
    /// `writable`
    #[account(mut, seeds = [b"data"], bump)]
    pub data: Account<DataAccount>,
    /// `readonly_signer`
    pub authority: Signer,
    /// `readonly`
    pub spectator: UncheckedAccount,
}

#[account]
pub struct DataAccount {
    pub value: u64,
    pub authority: Address,
}
