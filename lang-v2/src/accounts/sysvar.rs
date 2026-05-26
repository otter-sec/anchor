use {
    crate::AnchorAccount,
    core::{marker::PhantomData, ops::Deref},
    pinocchio::{account::AccountView, address::Address, sysvars::Sysvar as PinocchioSysvar},
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

/// Account wrapper for sysvars.
///
/// Validates that the passed account address matches `T::SYSVAR_ID`,
/// then reads the sysvar directly from the runtime via pinocchio's
/// `Sysvar::get()` syscall (account data is not deserialized).
///
/// ## `#[account(address = X @ MyErr)]` does NOT surface `MyErr`
///
/// `Sysvar<T>` validates the address against `T::SYSVAR_ID` inside `load`,
/// before any derive-level constraint hook. A mismatch surfaces as
/// `ProgramError::IncorrectProgramId`, never as the user's `@ MyErr` code.
/// If you need a custom error code on a sysvar address mismatch, use
/// `UncheckedAccount` and add `address = X @ MyErr` in the derive.
pub struct Sysvar<T: PinocchioSysvar + SysvarId + Copy> {
    view: AccountView,
    data: T,
    _phantom: PhantomData<T>,
}

impl<T: PinocchioSysvar + SysvarId + Copy> AnchorAccount for Sysvar<T> {
    type Data = T;

    fn load(view: AccountView, _program_id: &Address) -> Result<Self, ProgramError> {
        // Same chunked-compare rationale as `Program<T>::load`. See lib.rs.
        let id = T::SYSVAR_ID;
        if !crate::address_eq(view.address(), &id) {
            return Err(ProgramError::InvalidArgument);
        }
        // Use pinocchio's Sysvar::get() which reads directly from the runtime
        // via syscall, avoiding the need to deserialize from account data.
        let data = T::get().map_err(|_| ProgramError::UnsupportedSysvar)?;
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

impl<T: PinocchioSysvar + SysvarId + Copy> Deref for Sysvar<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T: PinocchioSysvar + SysvarId + Copy> AsRef<AccountView> for Sysvar<T> {
    fn as_ref(&self) -> &AccountView {
        &self.view
    }
}

impl<T: PinocchioSysvar + SysvarId + Copy> crate::ToCpiHandle for Sysvar<T> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        crate::AnchorAccount::cpi_handle(self)
    }
}

impl<T: PinocchioSysvar + SysvarId + Copy> crate::ToCpiHandleMut for Sysvar<T> {
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandle<'_>, solana_program_error::ProgramError> {
        crate::AnchorAccount::try_cpi_handle_mut(self)
    }
}

#[doc(hidden)]
impl<T: PinocchioSysvar + SysvarId + Copy> crate::IdlAccountType for Sysvar<T> {
    const __IDL_ADDRESS: Option<&'static str> = if T::IDL_ADDRESS.is_empty() {
        None
    } else {
        Some(T::IDL_ADDRESS)
    };
}
