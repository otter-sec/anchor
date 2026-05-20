use {
    crate::{accounts::view_wrapper_traits, AccountInitialize, AnchorAccount},
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
}

/// Init for `UncheckedAccount`: creates a zero-initialized account at
/// `view.address()` with the requested space, owned by `program_id`, then
/// returns the loaded view. Intended for the "init then CPI into a foreign
/// program that owns the account" pattern — pair with
/// `#[account(init, owner = foreign::ID, ...)]` so the new account is
/// handed off owned by the right program.
impl AccountInitialize for UncheckedAccount {
    type Params<'a> = ();

    #[inline(always)]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        program_id: &Address,
        _params: &(),
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        match signer_seeds {
            Some(seeds) => crate::create_account_signed(payer, account, space, program_id, seeds)?,
            None => crate::create_account(payer, account, space, program_id)?,
        }
        Ok(Self { view: *account })
    }
}

view_wrapper_traits!(UncheckedAccount);

#[doc(hidden)]
impl crate::IdlAccountType for UncheckedAccount {}
