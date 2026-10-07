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

type MaybeSpy = Option<SpyAccount>;
#[derive(Accounts)]
struct AliasedOptional {
    #[account(mut, constraint = present.account().is_writable())]
    present: MaybeSpy,
    #[account(mut, constraint = false)]
    absent: MaybeSpy,
}

#[test]
fn optional_aliases_use_the_same_constraint_and_mask_protocol() {
    use anchor_lang::{
        testing::{AccountBuffer, MIN_ACCOUNT_BUF},
        TryAccounts,
    };
    let present = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    present.init([2; 32], [0; 32], 0, false, true, false);
    let absent = AccountBuffer::<MIN_ACCOUNT_BUF>::new();
    absent.init(crate::ID.to_bytes(), [0; 32], 0, false, false, false);
    let views = [unsafe { present.view() }, unsafe { absent.view() }];
    let (accounts, _, ()) =
        AliasedOptional::try_accounts(&crate::ID, &views, None, 0, &[]).unwrap();
    assert!(accounts.present.is_some());
    assert!(accounts.absent.is_none());
    assert_eq!(AliasedOptional::MUT_MASK, [0; 4]);
    assert!(AliasedOptional::HAS_DYNAMIC_MUT_MASK);
    assert_eq!(accounts.active_mut_mask(), [1, 0, 0, 0]);
}
