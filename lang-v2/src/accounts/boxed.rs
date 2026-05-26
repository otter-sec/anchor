extern crate alloc;

use {
    crate::{AccountConstraint, AccountInitialize, AnchorAccount, Discriminator, Space},
    alloc::boxed::Box,
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

impl<T: AnchorAccount> AnchorAccount for Box<T> {
    type Data = T;
    const IS_SIGNER: bool = T::IS_SIGNER;
    const MIN_DATA_LEN: usize = T::MIN_DATA_LEN;

    fn load(view: AccountView) -> Result<Self, ProgramError> {
        T::load(view).map(Box::new)
    }

    /// # Safety
    ///
    /// See [`AnchorAccount::load_mut`] — caller must ensure no other live
    /// `&mut` to the same account data exists.
    unsafe fn load_mut(view: AccountView) -> Result<Self, ProgramError> {
        T::load_mut(view).map(Box::new)
    }

    /// # Safety
    ///
    /// See [`AnchorAccount::load_mut_after_init`] — caller must ensure no
    /// other live `&mut` to the same account data exists.
    unsafe fn load_mut_after_init(view: AccountView) -> Result<Self, ProgramError> {
        T::load_mut_after_init(view).map(Box::new)
    }

    fn account(&self) -> &AccountView {
        (**self).account()
    }

    fn exit(&mut self) -> pinocchio::ProgramResult {
        (**self).exit()
    }

    fn close(&mut self, destination: AccountView) -> pinocchio::ProgramResult {
        (**self).close(destination)
    }
}

impl<T: crate::ToCpiHandle + ?Sized> crate::ToCpiHandle for Box<T> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        self.as_ref().to_cpi_handle()
    }
}

impl<T: crate::ToCpiHandleMut + ?Sized> crate::ToCpiHandleMut for Box<T> {
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandleMut<'_>, solana_program_error::ProgramError> {
        self.as_mut().try_to_cpi_handle_mut()
    }
}

impl<T: crate::AccountRealloc> crate::AccountRealloc for Box<T> {
    #[inline(always)]
    fn realloc_account(
        &mut self,
        new_space: usize,
        payer: AccountView,
        zero: bool,
    ) -> pinocchio::ProgramResult {
        self.as_mut().realloc_account(new_space, payer, zero)
    }
}

#[doc(hidden)]
impl<T: crate::IdlAccountType> crate::IdlAccountType for Box<T> {
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = T::__IDL_ACCOUNT_ENTRY;
    const __IDL_TYPE_DEF: Option<&'static str> = T::__IDL_TYPE_DEF;
    fn __register_idl_deps(
        accounts: &mut ::alloc::vec::Vec<&'static str>,
        types: &mut ::alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

// ---------------------------------------------------------------------------
// Forward the init-time trait surface so `Box<Account<T>>` and
// `Box<BorshAccount<T>>` work with `#[account(init, …)]`, `#[account(zeroed)]`,
// `space = …` omitted, and namespaced constraints (`token::mint = …`, etc.).
//
// The derive reaches for these traits via UFCS on the field type — e.g.
// `<Box<Account<T>> as AccountInitialize>::create_and_initialize(…)` — so
// auto-deref on the receiver isn't sufficient; explicit forwards are required.
// ---------------------------------------------------------------------------

impl<T: AccountInitialize> AccountInitialize for Box<T> {
    type Params<'a> = T::Params<'a>;

    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        owner: &Address,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        T::create_and_initialize(payer, account, space, owner, params, signer_seeds).map(Box::new)
    }
}

impl<T: Space> Space for Box<T> {
    const INIT_SPACE: usize = T::INIT_SPACE;
}

impl<T: Discriminator> Discriminator for Box<T> {
    const DISCRIMINATOR: &'static [u8] = T::DISCRIMINATOR;
}

impl<C, T> AccountConstraint<Box<T>> for C
where
    C: AccountConstraint<T>,
{
    type Value = <C as AccountConstraint<T>>::Value;

    #[inline(always)]
    fn init(account: &mut Box<T>, value: &Self::Value) -> Result<(), ProgramError> {
        C::init(account.as_mut(), value)
    }

    #[inline(always)]
    fn check(account: &Box<T>, value: &Self::Value) -> Result<(), ProgramError> {
        C::check(account.as_ref(), value)
    }

    #[inline(always)]
    fn update(account: &mut Box<T>, value: &Self::Value) -> Result<(), ProgramError> {
        C::update(account.as_mut(), value)
    }

    #[inline(always)]
    fn exit(account: &mut Box<T>, value: &Self::Value) -> Result<(), ProgramError> {
        C::exit(account.as_mut(), value)
    }
}
