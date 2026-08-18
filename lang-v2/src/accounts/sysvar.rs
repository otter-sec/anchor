use {
    crate::{require, AnchorAccount},
    core::ops::Deref,
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

/// How [`Sysvar<T>`] obtains `T`'s value once the account address checks out.
///
/// Split out of [`SysvarId`] because the two sysvar families read differently:
/// `Clock` / `Rent` come from the `sol_get_sysvar` syscall and never touch
/// account data, while `Instructions` has no syscall at all and must be read
/// out of the account's data buffer.
///
/// A blanket `impl<T: PinocchioSysvar> SysvarLoad for T` is not possible: it
/// would overlap the [`Instructions`] impl, and rustc cannot prove
/// `Instructions: !PinocchioSysvar` (negative reasoning about a foreign trait
/// on a foreign type). Syscall-backed sysvars therefore get an explicit impl
/// each, via `impl_syscall_sysvar!`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a sysvar Anchor can load",
    label = "unsupported sysvar",
    note = "supported: `Clock`, `Rent`, `Instructions`. A sysvar needs both `SysvarId` (its \
            well-known address) and `SysvarLoad` (how to read its value)."
)]
pub trait SysvarLoad: SysvarId + Sized {
    /// Read the sysvar's value.
    ///
    /// [`Sysvar<T>::load`] has already verified that
    /// `view.address() == Self::SYSVAR_ID` before this runs, so implementors
    /// may skip any address check of their own.
    ///
    /// [`Sysvar<T>::load`]: AnchorAccount::load
    fn read(view: &AccountView) -> Result<Self, ProgramError>;
}

/// Registers a sysvar that the runtime exposes through `sol_get_sysvar`.
///
/// `read` ignores the account view entirely — the value comes from the
/// syscall, so the account's data is never deserialized.
macro_rules! impl_syscall_sysvar {
    ($ty:ty, $id:expr, $idl:literal) => {
        impl SysvarId for $ty {
            const SYSVAR_ID: Address = $id;
            const IDL_ADDRESS: &'static str = $idl;
        }

        impl SysvarLoad for $ty {
            #[inline(always)]
            fn read(_view: &AccountView) -> Result<Self, ProgramError> {
                <$ty as PinocchioSysvar>::get().map_err(|_| ProgramError::UnsupportedSysvar)
            }
        }
    };
}

impl_syscall_sysvar!(
    pinocchio::sysvars::clock::Clock,
    pinocchio::sysvars::clock::CLOCK_ID,
    "SysvarC1ock11111111111111111111111111111111"
);

impl_syscall_sysvar!(
    pinocchio::sysvars::rent::Rent,
    pinocchio::sysvars::rent::RENT_ID,
    "SysvarRent111111111111111111111111111111111"
);

// FIXME: Add `EpochSchedule`: https://github.com/anza-xyz/pinocchio/pull/411

// Deliberately generic over `T`: the address and IDL string are the same for
// every instantiation, and `tests-v2/tests/sysvar_idl.rs` asserts on
// `Instructions<&'static [u8]>`. Only the `Instructions` alias below — the one
// instantiation that can outlive `load` — gets a `SysvarLoad` impl.
impl<T: Deref<Target = [u8]>> SysvarId for pinocchio::sysvars::instructions::Instructions<T> {
    const SYSVAR_ID: Address = pinocchio::sysvars::instructions::INSTRUCTIONS_ID;
    const IDL_ADDRESS: &'static str = "Sysvar1nstructions1111111111111111111111111";
}

/// The instructions sysvar — the entry point for instruction introspection.
///
/// Instantiates pinocchio's `Instructions<T>` at the one `T` that can outlive
/// [`AnchorAccount::load`]: a `'static` borrow guard over the account's data.
///
/// Use it as `Sysvar<Instructions>` in a `#[derive(Accounts)]` struct, then
/// reach the introspection methods through the wrapper's `Deref`:
///
/// ```ignore
/// #[derive(Accounts)]
/// pub struct Introspect {
///     pub instructions: Sysvar<Instructions>,
/// }
///
/// let previous = ctx.accounts.instructions.get_instruction_relative(-1)?;
/// let caller = previous.get_program_id();
/// ```
///
/// Unlike `Clock` / `Rent`, there is no syscall for this sysvar: the account
/// must be passed in the transaction, and the wrapper holds a shared borrow of
/// its data for as long as it is alive.
pub type Instructions = pinocchio::sysvars::instructions::Instructions<Ref<'static, [u8]>>;

impl SysvarLoad for Instructions {
    #[inline(always)]
    fn read(view: &AccountView) -> Result<Self, ProgramError> {
        // A well-formed instructions sysvar is at minimum `[u16 num = 0]` +
        // `[u16 current_index]`. The address check in `Sysvar::load` already
        // guarantees this is the genuine runtime-populated sysvar; the guard
        // only stops a hand-rolled mock view from underflowing the pointer
        // arithmetic in `load_current_index`.
        #[cfg(feature = "guardrails")]
        require!(view.data_len() >= 4, ProgramError::AccountDataTooSmall);

        let data_ref = view.try_borrow()?;
        // SAFETY: the AccountView's data pointer is valid for the entire
        // instruction (Solana runtime guarantee), and `Ref` stores raw pointers
        // into runtime memory rather than into `view` — so moving `view` into
        // `Sysvar<T>` afterwards does not invalidate it. Holding the guard
        // prevents subsequent mutable borrows of the same account. Same
        // reasoning as `SerializedAccount::load`.
        let guard: Ref<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        // SAFETY: `Sysvar::load` already verified the address against
        // `INSTRUCTIONS_ID`, which is exactly what pinocchio's
        // `TryFrom<&AccountView>` checks. Going through `new_unchecked` avoids
        // redoing that compare and lets us transmute the `Ref` alone rather
        // than the whole `Instructions<_>`.
        Ok(unsafe { Self::new_unchecked(guard) })
    }
}

/// Account wrapper for sysvars.
///
/// Validates that the passed account address matches `T::SYSVAR_ID`, then
/// defers to [`SysvarLoad::read`] for the value. For `Clock` / `Rent` that
/// reads directly from the runtime via pinocchio's `Sysvar::get()` syscall and
/// never touches account data; for [`Instructions`] it borrows the account's
/// data and holds that shared borrow for the wrapper's lifetime.
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
        let data = T::read(&view)?;
        Ok(Self { view, data })
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
