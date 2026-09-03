//! Smoke test for optional mutable custom account wrappers.
//!
//! The behavioral regression is covered end-to-end in `tests-v2`; this file
//! just keeps a focused derive example in `lang-v2`.

use {
    anchor_lang::{Accounts, AnchorAccount},
    core::{mem::size_of, ops::Deref},
    pinocchio::account::AccountView,
    solana_program_error::ProgramError,
};

anchor_lang::declare_id!("11111111111111111111111111111111");

struct SpyAccount {
    view: AccountView,
}

impl Deref for SpyAccount {
    type Target = AccountView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl AnchorAccount for SpyAccount {
    type Data = AccountView;

    fn load(view: AccountView) -> Result<Self, ProgramError> {
        Ok(Self { view })
    }

    fn account(&self) -> &AccountView {
        &self.view
    }
}

#[allow(dead_code)]
#[derive(Accounts)]
struct OptionalSpyAccounts {
    #[account(mut)]
    a: Option<SpyAccount>,
    #[account(mut)]
    b: Option<SpyAccount>,
}

#[test]
fn optional_mut_duplicate_derive_smoke() {
    let _ = size_of::<OptionalSpyAccounts>();
}
