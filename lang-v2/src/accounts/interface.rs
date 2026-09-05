use {
    crate::{require, AnchorAccount, Ids},
    core::{marker::PhantomData, ops::Deref},
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

/// Program account wrapper that accepts any address declared by `T::ids()`.
pub struct Interface<T: Ids> {
    view: AccountView,
    _phantom: PhantomData<T>,
}

impl<T: Ids> Interface<T> {
    /// Returns the account's address.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }
}

impl<T: Ids> AnchorAccount for Interface<T> {
    type Data = AccountView;

    #[inline(always)]
    fn load(view: AccountView) -> Result<Self, ProgramError> {
        #[cfg(feature = "guardrails")]
        require!(view.executable(), ConstraintExecutable);
        require!(
            T::ids()
                .iter()
                .any(|id| crate::address_eq(view.address(), id)),
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

impl<T: Ids> Deref for Interface<T> {
    type Target = AccountView;

    #[inline(always)]
    fn deref(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Ids> AsRef<AccountView> for Interface<T> {
    #[inline(always)]
    fn as_ref(&self) -> &AccountView {
        &self.view
    }
}

impl<T: Ids> AsRef<Address> for Interface<T> {
    #[inline(always)]
    fn as_ref(&self) -> &Address {
        self.view.address()
    }
}

impl<T: Ids> crate::ToCpiHandle for Interface<T> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        crate::AnchorAccount::cpi_handle(self)
    }
}

impl<T: Ids> crate::ToCpiHandleMut for Interface<T> {
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandleMut<'_>, solana_program_error::ProgramError> {
        crate::AnchorAccount::try_cpi_handle_mut(self)
    }
}

#[doc(hidden)]
impl<T: Ids> crate::IdlAccountType for Interface<T> {}
