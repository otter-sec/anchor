use {
    crate::{require, AnchorAccount, Id},
    core::{marker::PhantomData, ops::Deref},
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

/// Program account wrapper. Validates the account address matches `T::id()`.
///
/// ## `#[account(address = X @ MyErr)]` does NOT surface `MyErr`
///
/// `Program<T>` validates the address against `T::id()` inside `load`,
/// before any derive-level constraint hook. A mismatch surfaces as
/// `ProgramError::IncorrectProgramId`, never as the user's `@ MyErr` code.
/// If you need a custom error on a program-id mismatch, use
/// `UncheckedAccount` and add the address check via `address = X @ MyErr`
/// in the derive — that becomes the authoritative validation.
pub struct Program<T: Id> {
    view: AccountView,
    _phantom: PhantomData<T>,
}

impl<T: Id> Program<T> {
    /// Returns the account's address.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }
}

impl<T: Id> AnchorAccount for Program<T> {
    type Data = AccountView;
    #[inline(always)]
    fn load(view: AccountView) -> Result<Self, ProgramError> {
        #[cfg(feature = "guardrails")]
        require!(view.executable(), ProgramError::InvalidAccountData);
        let id = T::id();
        require!(
            crate::address_eq(view.address(), &id),
            ProgramError::IncorrectProgramId
        );
        Ok(Self {
            view,
            _phantom: PhantomData,
        })
    }
    #[inline(always)]
    fn account(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Id> Deref for Program<T> {
    type Target = AccountView;
    fn deref(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Id> AsRef<AccountView> for Program<T> {
    fn as_ref(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Id> AsRef<Address> for Program<T> {
    fn as_ref(&self) -> &Address {
        self.view.address()
    }
}

impl<T: Id> crate::ToCpiHandle for Program<T> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        crate::AnchorAccount::cpi_handle(self)
    }
}

impl<T: Id> crate::ToCpiHandleMut for Program<T> {
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandleMut<'_>, solana_program_error::ProgramError> {
        crate::AnchorAccount::try_cpi_handle_mut(self)
    }
}

#[doc(hidden)]
impl<T: Id> crate::IdlAccountType for Program<T> {
    // `Id::IDL_ADDRESS` defaults to `""`; convert empty → None so unknown
    // program markers elide the `address` field instead of emitting a bogus
    // blank string.
    const __IDL_ADDRESS: Option<&'static str> = if T::IDL_ADDRESS.is_empty() {
        None
    } else {
        Some(T::IDL_ADDRESS)
    };
}
