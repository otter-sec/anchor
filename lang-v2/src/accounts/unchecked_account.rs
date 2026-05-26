use {
    crate::{accounts::view_wrapper_traits, AccountInitialize, AnchorAccount, ForeignOwnerInit},
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
    fn load(view: AccountView) -> Result<Self, ProgramError> {
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

impl AccountInitialize for UncheckedAccount {
    type Params<'a> = ();

    #[inline(always)]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        owner: &Address,
        _params: &(),
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        match signer_seeds {
            Some(seeds) => crate::create_account_signed(payer, account, space, owner, seeds)?,
            None => crate::create_account(payer, account, space, owner)?,
        }
        Self::load(*account)
    }
}

impl ForeignOwnerInit for UncheckedAccount {}

view_wrapper_traits!(UncheckedAccount);

#[doc(hidden)]
impl crate::IdlAccountType for UncheckedAccount {}
