mod borsh_account;
mod boxed;
mod interface;
mod program;
mod serialized_account;
mod signer;
mod slab;
mod slab_hooks;
mod system_account;
mod sysvar;
mod unchecked_account;

pub use {
    crate::AccountInitialize,
    borsh_account::{BorshAccount, BorshSerializer},
    interface::Interface,
    program::Program,
    serialized_account::{AnchorAccountSerialize, SerializedAccount},
    signer::Signer,
    slab::{HeaderOnly, Slab},
    slab_hooks::{SlabInit, SlabSchema},
    system_account::SystemAccount,
    sysvar::{Sysvar, SysvarId},
    unchecked_account::UncheckedAccount,
};

/// Anchor account with a typed header and no trailing items.
///
/// Alias for `Slab<T, HeaderOnly>` — layout is `[disc][T]`. Shares all of
/// Slab's validation, borrow-tracking, init, and close machinery.
/// `HeaderOnly` is a ZST that doesn't impl `Pod`, so tail-only methods
/// (`len`, `push`, `as_slice`, ...) are "method not found" compile errors.
/// Layout is byte-identical to the pre-Slab `Account<T>` (no migration).
///
/// For accounts with a length-prefixed tail, use `Slab<H, T>` directly.
///
/// ## `#[account(owner = X @ MyErr)]` does NOT surface `MyErr`
///
/// Owner/discriminator validation runs inside `load`/`load_mut`, before
/// any derive-level constraint hook. A mismatch surfaces as
/// `ProgramError::IllegalOwner`, not the user's `@ MyErr` code. For a
/// custom error, use `UncheckedAccount` with derive-level `owner = X @ MyErr`.
pub type Account<T> = slab::Slab<T, HeaderOnly>;

/// Generic interface account alias.
///
/// Token-interface semantics are carried by the `T` type itself, for example
/// `anchor_spl_v2::token_interface::TokenAccount`.
pub type InterfaceAccount<T> = Account<T>;

/// Generates `Deref<Target=AccountView>` + `AsRef<AccountView>` + `AsRef<Address>`
/// for a view wrapper that stores its `AccountView` in a field named `view`.
///
/// Covers only the mechanical trait delegation — validation logic and any
/// extra inherent methods (e.g. `address()`) still live in the concrete type's
/// own file. Not used by `Account<T>` / `BorshAccount<T>` (non-`AccountView`
/// `Deref::Target`) or `Program<T>` (generic bounds).
macro_rules! view_wrapper_traits {
    ($Type:ty) => {
        impl core::ops::Deref for $Type {
            type Target = pinocchio::account::AccountView;
            #[inline(always)]
            fn deref(&self) -> &pinocchio::account::AccountView {
                &self.view
            }
        }
        impl AsRef<pinocchio::account::AccountView> for $Type {
            #[inline(always)]
            fn as_ref(&self) -> &pinocchio::account::AccountView {
                &self.view
            }
        }
        impl AsRef<pinocchio::address::Address> for $Type {
            #[inline(always)]
            fn as_ref(&self) -> &pinocchio::address::Address {
                self.view.address()
            }
        }
        impl crate::ToCpiHandle for $Type {
            #[inline(always)]
            fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
                crate::AnchorAccount::cpi_handle(self)
            }
        }

        impl crate::ToCpiHandleMut for $Type {
            #[inline(always)]
            fn try_to_cpi_handle_mut(
                &mut self,
            ) -> Result<crate::CpiHandle<'_>, solana_program_error::ProgramError> {
                crate::AnchorAccount::try_cpi_handle_mut(self)
            }
        }
    };
}
pub(crate) use view_wrapper_traits;
