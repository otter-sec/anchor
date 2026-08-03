//! Third-party-style custom constraints exercising each of the four
//! `AccountConstraint` methods (`init`, `check`, `update`, `exit`).
//!
//! The `counter_ns` module defines four constraint markers, each
//! overriding exactly one method. Handlers drive each constraint via
//! the `#[account(...)]` syntax, and the integration tests in
//! `tests/custom_constraints.rs` assert the resulting on-chain state
//! to pin down WHERE in the lifecycle the derive routes each call.

extern crate alloc;

use anchor_lang_v2::prelude::*;

declare_id!("CC111111111111111111111111111111111111111111");

#[program]
pub mod custom_constraints {
    use super::*;

    /// `counter_ns::init_value = 5` on an `init` field.
    /// `InitValueConstraint::init` stamps `counter.value = 5` after
    /// BorshAccount creates + zero-fills the account.
    pub fn handle_init(_ctx: &mut Context<HandleInit>) -> Result<()> {
        Ok(())
    }

    /// `counter_ns::min_value = 10` on a non-init field.
    /// `MinValueConstraint::check` asserts `counter.value >= 10`.
    pub fn handle_check(_ctx: &mut Context<HandleCheck>) -> Result<()> {
        Ok(())
    }

    /// Access control observes the initialized value (`5`), then
    /// `update(counter_ns::set_value = 42)` runs before this body.
    #[access_control(require_counter_before_update(ctx))]
    pub fn handle_update(ctx: &mut Context<HandleUpdate>) -> Result<()> {
        require!(ctx.accounts.counter.value == 42, WErr::BadValue);
        Ok(())
    }

    /// Initialize the authority record used by the update/access-control
    /// regression. The payer is the initial authorized signer.
    pub fn handle_authority_init(ctx: &mut Context<HandleAuthorityInit>) -> Result<()> {
        ctx.accounts.counter.value = address_tag(ctx.accounts.payer.address());
        Ok(())
    }

    /// Rotate the stored authority. Access control must compare `actor`
    /// against the original stored authority before the generated update
    /// writes `next_authority`; the body must then observe the new authority.
    #[access_control(require_current_authority(ctx))]
    pub fn handle_authority_update(ctx: &mut Context<HandleAuthorityUpdate>) -> Result<()> {
        require!(
            ctx.accounts.counter.value == address_tag(ctx.accounts.next_authority.address()),
            WErr::BadValue
        );
        Ok(())
    }

    /// `counter_ns::bump_on_exit = 1` on a `mut` field.
    /// `BumpOnExitConstraint::exit` adds 1 to `counter.value` during
    /// `exit_accounts`. The integration test dispatches an otherwise
    /// no-op handler and checks the persisted value afterwards.
    pub fn handle_exit_bump(_ctx: &mut Context<HandleExitBump>) -> Result<()> {
        Ok(())
    }

    /// `init_if_needed` with `counter_ns::init_value = 5` —
    /// exercises that on the create branch, `init` fires, and on the
    /// exist branch, `check` fires (no-op for this constraint but the
    /// call is still emitted). Pairs with a separate non-init
    /// constraint `counter_ns::min_value` to prove the check path is
    /// reached.
    pub fn handle_init_if_needed(_ctx: &mut Context<HandleInitIfNeeded>) -> Result<()> {
        Ok(())
    }

    /// Boxed variant of `handle_init`, proving `AccountConstraint::init`
    /// forwards through `Box<T>` on an `init` field.
    pub fn handle_boxed_init(_ctx: &mut Context<HandleBoxedInit>) -> Result<()> {
        Ok(())
    }

    /// Boxed variant of `handle_exit_bump`, proving `AccountConstraint::exit`
    /// forwards through `Box<T>`.
    pub fn handle_boxed_exit_bump(_ctx: &mut Context<HandleBoxedExitBump>) -> Result<()> {
        Ok(())
    }

    /// Close a boxed counter to exercise `AccountClose::close` forwarding.
    pub fn handle_boxed_close(_ctx: &mut Context<HandleBoxedClose>) -> Result<()> {
        Ok(())
    }

    /// Optional variant of `handle_init`, proving `AccountConstraint::init`
    /// fires when `Option<T>` creates `Some(T)`.
    pub fn handle_optional_init(_ctx: &mut Context<HandleOptionalInit>) -> Result<()> {
        Ok(())
    }

    /// Optional boxed variant of `handle_init`.
    pub fn handle_optional_boxed_init(_ctx: &mut Context<HandleOptionalBoxedInit>) -> Result<()> {
        Ok(())
    }

    /// Optional `init_if_needed` variant. The create branch must run `init`
    /// before the later `check`, and the exist branch must only run `check`.
    pub fn handle_optional_init_if_needed(
        _ctx: &mut Context<HandleOptionalInitIfNeeded>,
    ) -> Result<()> {
        Ok(())
    }

    /// `update(...)` should not mutate `fresh` until later field
    /// validations have completed. The integration test wires a later
    /// `witness` constraint that expects the pre-update value.
    pub fn handle_update_after_later_validation(
        _ctx: &mut Context<HandleUpdateAfterLaterValidation>,
    ) -> Result<()> {
        Ok(())
    }
}

fn address_tag(address: &Address) -> u64 {
    let bytes = address.as_array();
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

fn require_counter_before_update(ctx: &Context<HandleUpdate>) -> Result<()> {
    require!(ctx.accounts.counter.value == 5, WErr::Unauthorized);
    Ok(())
}

fn require_current_authority(ctx: &Context<HandleAuthorityUpdate>) -> Result<()> {
    require!(
        ctx.accounts.counter.value == address_tag(ctx.accounts.actor.address()),
        WErr::Unauthorized
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Account data type — a plain Borsh-serialised counter.
// ---------------------------------------------------------------------------

#[account(borsh)]
pub struct Counter {
    pub value: u64,
}

// ---------------------------------------------------------------------------
// Custom constraint namespace. Each marker overrides exactly one of the
// four `AccountConstraint` methods to make routing observable.
// ---------------------------------------------------------------------------

pub mod counter_ns {
    use {
        super::{address_tag, Counter},
        anchor_lang_v2::{accounts::BorshAccount, AccountConstraint, AccountView},
        solana_program_error::ProgramError,
    };

    /// `counter_ns::init_value = N` — stamps `counter.value = N`
    /// during the init phase (fires on `init` and on the create branch
    /// of `init_if_needed`).
    pub struct InitValueConstraint;

    impl AccountConstraint<BorshAccount<Counter>> for InitValueConstraint {
        type Value = u64;

        fn init(account: &mut BorshAccount<Counter>, value: &u64) -> Result<(), ProgramError> {
            account.value = *value;
            Ok(())
        }
    }

    /// `counter_ns::min_value = N` — asserts `counter.value >= N`.
    pub struct MinValueConstraint;

    impl AccountConstraint<BorshAccount<Counter>> for MinValueConstraint {
        type Value = u64;

        fn check(account: &BorshAccount<Counter>, min: &u64) -> Result<(), ProgramError> {
            if account.value < *min {
                return Err(ProgramError::InvalidAccountData);
            }
            Ok(())
        }
    }

    /// `counter_ns::set_value = N` — writes `counter.value = N`.
    /// Emitted only when paired with the `update(...)` wrapper.
    pub struct SetValueConstraint;

    impl AccountConstraint<BorshAccount<Counter>> for SetValueConstraint {
        type Value = u64;

        fn update(account: &mut BorshAccount<Counter>, value: &u64) -> Result<(), ProgramError> {
            account.value = *value;
            Ok(())
        }
    }

    /// `counter_ns::set_authority = account` — stores a compact identity tag
    /// for the supplied account. Used to prove access control runs before an
    /// attacker-controlled authority update.
    pub struct SetAuthorityConstraint;

    impl AccountConstraint<BorshAccount<Counter>> for SetAuthorityConstraint {
        type Value = AccountView;

        fn update(
            account: &mut BorshAccount<Counter>,
            value: &AccountView,
        ) -> Result<(), ProgramError> {
            account.value = address_tag(value.address());
            Ok(())
        }
    }

    /// `counter_ns::bump_on_exit = N` — adds `N` to `counter.value`
    /// during `exit_accounts`. The exit hook runs only on successful
    /// instructions, so a handler that errors must NOT see the bump.
    pub struct BumpOnExitConstraint;

    impl AccountConstraint<BorshAccount<Counter>> for BumpOnExitConstraint {
        type Value = u64;

        fn exit(account: &mut BorshAccount<Counter>, bump: &u64) -> Result<(), ProgramError> {
            account.value = account.value.saturating_add(*bump);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Accounts structs
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct HandleInit {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + 8, // disc + u64
        seeds = [b"counter"],
        bump,
        counter_ns::init_value = 5u64,
    )]
    pub counter: BorshAccount<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleCheck {
    #[account(
        seeds = [b"counter"],
        bump,
        counter_ns::min_value = 10u64,
    )]
    pub counter: BorshAccount<Counter>,
}

#[derive(Accounts)]
pub struct HandleUpdate {
    #[account(
        mut,
        seeds = [b"counter"],
        bump,
        update(counter_ns::set_value = 42u64),
    )]
    pub counter: BorshAccount<Counter>,
}

#[derive(Accounts)]
pub struct HandleAuthorityInit {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + 8,
        seeds = [b"authority-counter"],
        bump,
    )]
    pub counter: BorshAccount<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleAuthorityUpdate {
    #[account(
        mut,
        seeds = [b"authority-counter"],
        bump,
        update(counter_ns::set_authority = next_authority),
    )]
    pub counter: BorshAccount<Counter>,
    pub actor: Signer,
    pub next_authority: UncheckedAccount,
}

#[derive(Accounts)]
pub struct HandleExitBump {
    #[account(
        mut,
        seeds = [b"counter"],
        bump,
        counter_ns::bump_on_exit = 1u64,
    )]
    pub counter: BorshAccount<Counter>,
}

#[derive(Accounts)]
pub struct HandleInitIfNeeded {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + 8,
        seeds = [b"counter"],
        bump,
        counter_ns::init_value = 5u64,
        counter_ns::min_value = 1u64,
    )]
    pub counter: BorshAccount<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleBoxedInit {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + 8,
        seeds = [b"boxed-counter"],
        bump,
        counter_ns::init_value = 9u64,
    )]
    pub counter: Box<BorshAccount<Counter>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleBoxedExitBump {
    #[account(
        mut,
        seeds = [b"boxed-counter"],
        bump,
        counter_ns::bump_on_exit = 2u64,
    )]
    pub counter: Box<BorshAccount<Counter>>,
}

#[derive(Accounts)]
pub struct HandleBoxedClose {
    #[account(mut, close = receiver, seeds = [b"boxed-counter"], bump)]
    pub counter: Box<BorshAccount<Counter>>,
    #[account(mut)]
    pub receiver: SystemAccount,
}

#[derive(Accounts)]
pub struct HandleOptionalInit {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + 8,
        seeds = [b"optional-counter"],
        bump,
        counter_ns::init_value = 7u64,
    )]
    pub counter: Option<BorshAccount<Counter>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleOptionalBoxedInit {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8 + 8,
        seeds = [b"optional-boxed-counter"],
        bump,
        counter_ns::init_value = 11u64,
    )]
    pub counter: Option<Box<BorshAccount<Counter>>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleOptionalInitIfNeeded {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + 8,
        seeds = [b"optional-init-if-needed-counter"],
        bump,
        counter_ns::init_value = 13u64,
        counter_ns::min_value = 1u64,
    )]
    pub counter: Option<BorshAccount<Counter>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct HandleUpdateAfterLaterValidation {
    #[account(
        zeroed,
        counter_ns::min_value = 0u64,
        update(counter_ns::set_value = 7u64)
    )]
    pub fresh: BorshAccount<Counter>,
    #[account(constraint = fresh.value == 0 @ WErr::BadValue)]
    pub witness: UncheckedAccount,
}

#[error_code]
pub enum WErr {
    #[msg("bad value")]
    BadValue,
    #[msg("unauthorized")]
    Unauthorized,
}
