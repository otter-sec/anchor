use {
    crate::{accounts::view_wrapper_traits, AnchorAccount},
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

pub struct UncheckedAccount {
    view: AccountView,
}

impl UncheckedAccount {
    /// Returns the account's address.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }
}

impl AnchorAccount for UncheckedAccount {
    type Data = AccountView;
    #[inline(always)]
    fn load(view: AccountView, _program_id: &Address) -> Result<Self, ProgramError> {
        Ok(Self { view })
    }
    #[inline(always)]
    fn account(&self) -> &AccountView {
        &self.view
    }

    fn close(&mut self, _destination: AccountView) -> pinocchio::ProgramResult {
        Err(ProgramError::InvalidArgument)
    }
}

view_wrapper_traits!(UncheckedAccount);

#[doc(hidden)]
impl crate::IdlAccountType for UncheckedAccount {}
