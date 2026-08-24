use {
    crate::{require, AnchorAccount},
    core::{marker::PhantomData, ops::Deref},
    pinocchio::{
        account::{AccountView, Ref},
        address::Address,
        sysvars::Sysvar as PinocchioSysvar,
    },
    solana_program_error::ProgramError,
};

/// Trait that connects a pinocchio sysvar type to its well-known address.
///
/// `IDL_ADDRESS` is the base58 string surfaced through
/// `IdlAccountType::__IDL_ADDRESS` at IDL emission time. Defaults to an
/// empty string — sysvars without a well-known address (or ones whose
/// address isn't meaningful in the IDL) elide the field.
pub trait SysvarId {
    /// The sysvar's well-known account address.
    const SYSVAR_ID: Address;
    /// Well-known base58 address for IDL emission. Empty string → no
    /// `address` emission at the `Program<T>` / `Sysvar<T>` IDL site.
    const IDL_ADDRESS: &'static str = "";
}

impl SysvarId for pinocchio::sysvars::clock::Clock {
    const SYSVAR_ID: Address = pinocchio::sysvars::clock::CLOCK_ID;
    const IDL_ADDRESS: &'static str = "SysvarC1ock11111111111111111111111111111111";
}

impl<T: Deref<Target = [u8]>> SysvarId for pinocchio::sysvars::instructions::Instructions<T> {
    const SYSVAR_ID: Address = pinocchio::sysvars::instructions::INSTRUCTIONS_ID;
    const IDL_ADDRESS: &'static str = "Sysvar1nstructions1111111111111111111111111";
}

impl SysvarId for pinocchio::sysvars::rent::Rent {
    const SYSVAR_ID: Address = pinocchio::sysvars::rent::RENT_ID;
    const IDL_ADDRESS: &'static str = "SysvarRent111111111111111111111111111111111";
}

// FIXME: Add `EpochSchedule`: https://github.com/anza-xyz/pinocchio/pull/411

/// Concrete pinocchio [`Instructions`](pinocchio::sysvars::instructions::Instructions)
/// handle for use as `Sysvar<SysvarInstructions>`.
///
/// Named to avoid colliding with the common program-local `Instructions`
/// module / enum when `use anchor_lang_v2::prelude::*` is in scope.
///
/// The instructions sysvar is account-data backed (no `Sysvar::get` syscall),
/// so Anchor binds it to a `'static` borrow of the account buffer — the same
/// pattern [`super::SerializedAccount`] uses for its `Ref` guard.
pub type SysvarInstructions =
    pinocchio::sysvars::instructions::Instructions<Ref<'static, [u8]>>;

/// How [`Sysvar<T>`] materializes `T` after the account address is validated.
///
/// Syscall-backed sysvars (`Clock`, `Rent`) ignore account bytes and call
/// [`PinocchioSysvar::get`]. The instructions sysvar reads the supplied
/// account data instead.
pub trait SysvarLoad: SysvarId + Sized {
    fn load_data(view: &AccountView) -> Result<Self, ProgramError>;
}

impl SysvarLoad for pinocchio::sysvars::clock::Clock {
    #[inline(always)]
    fn load_data(_view: &AccountView) -> Result<Self, ProgramError> {
        <Self as PinocchioSysvar>::get().map_err(|_| ProgramError::UnsupportedSysvar)
    }
}

impl SysvarLoad for pinocchio::sysvars::rent::Rent {
    #[inline(always)]
    fn load_data(_view: &AccountView) -> Result<Self, ProgramError> {
        <Self as PinocchioSysvar>::get().map_err(|_| ProgramError::UnsupportedSysvar)
    }
}

impl SysvarLoad for SysvarInstructions {
    #[inline(always)]
    fn load_data(view: &AccountView) -> Result<Self, ProgramError> {
        let data_ref = view.try_borrow()?;
        // SAFETY: `AccountView` data is valid for the instruction lifetime.
        // `Sysvar` retains `view`, and the `Ref` guard keeps the borrow alive
        // for as long as this value exists (mirrors `SerializedAccount`'s
        // immutable load path).
        let guard: Ref<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        // SAFETY: caller (`Sysvar::load`) already checked `T::SYSVAR_ID`.
        Ok(unsafe {
            pinocchio::sysvars::instructions::Instructions::new_unchecked(guard)
        })
    }
}

/// Account wrapper for sysvars.
///
/// Validates that the passed account address matches `T::SYSVAR_ID`, then
/// loads `T` via [`SysvarLoad`]: syscall `get()` for `Clock` / `Rent`, or
/// a borrow of the account data for [`SysvarInstructions`].
///
/// ## `#[account(address = X @ MyErr)]` does NOT surface `MyErr`
///
/// `Sysvar<T>` validates the address against `T::SYSVAR_ID` inside `load`,
/// before any derive-level constraint hook. A mismatch surfaces as
/// `ProgramError::InvalidArgument`, never as the user's `@ MyErr` code.
/// If you need a custom error code on a sysvar address mismatch, use
/// `UncheckedAccount` and add `address = X @ MyErr` in the derive.
pub struct Sysvar<T: SysvarLoad> {
    view: AccountView,
    data: T,
    _phantom: PhantomData<T>,
}

impl<T: SysvarLoad> AnchorAccount for Sysvar<T> {
    type Data = T;

    fn load(view: AccountView) -> Result<Self, ProgramError> {
        // Same chunked-compare rationale as `Program<T>::load`. See lib.rs.
        let id = T::SYSVAR_ID;
        require!(
            crate::address_eq(view.address(), &id),
            ProgramError::InvalidArgument
        );
        let data = T::load_data(&view)?;
        Ok(Self {
            view,
            data,
            _phantom: PhantomData,
        })
    }

    fn account(&self) -> &AccountView {
        &self.view
    }
}

impl<T: SysvarLoad> Deref for Sysvar<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: SysvarLoad> AsRef<AccountView> for Sysvar<T> {
    fn as_ref(&self) -> &AccountView {
        &self.view
    }
}

impl<T: SysvarLoad> crate::ToCpiHandle for Sysvar<T> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        crate::AnchorAccount::cpi_handle(self)
    }
}

impl<T: SysvarLoad> crate::ToCpiHandleMut for Sysvar<T> {
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandleMut<'_>, solana_program_error::ProgramError> {
        crate::AnchorAccount::try_cpi_handle_mut(self)
    }
}

#[doc(hidden)]
impl<T: SysvarLoad> crate::IdlAccountType for Sysvar<T> {
    const __IDL_ADDRESS: Option<&'static str> = if T::IDL_ADDRESS.is_empty() {
        None
    } else {
        Some(T::IDL_ADDRESS)
    };
}
