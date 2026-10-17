//! Compatibility for the heap-backed account wrapper.
//!
//! This module is available only with the explicit `alloc` feature. A
//! no-alloc build does not expose the `Box<T>` account surface, while legacy
//! alloc-enabled programs retain their existing behavior.

#![cfg(all(
    feature = "alloc",
    any(target_os = "solana", not(target_arch = "bpf")),
))]

use {
    crate::{AccountConstraint, AccountInitialize, AnchorAccount, Discriminator, ForeignOwnerInit, Space},
    alloc::boxed::Box,
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

impl<T: AnchorAccount> AnchorAccount for Box<T> {
    type Data = T;
    const IS_SIGNER: bool = T::IS_SIGNER;
    const MIN_DATA_LEN: usize = T::MIN_DATA_LEN;
    const RELAX_READONLY_CPI_BORROW_FROM_MUT: bool = T::RELAX_READONLY_CPI_BORROW_FROM_MUT;
    fn load(view: AccountView) -> Result<Self, ProgramError> { T::load(view).map(Box::new) }
    unsafe fn load_mut(view: AccountView) -> Result<Self, ProgramError> { T::load_mut(view).map(Box::new) }
    unsafe fn load_mut_after_init(view: AccountView) -> Result<Self, ProgramError> {
        T::load_mut_after_init(view).map(Box::new)
    }
    fn account(&self) -> &AccountView { (**self).account() }
    fn cpi_handle(&self) -> crate::CpiHandle<'_> { T::cpi_handle(self.as_ref()) }
    fn try_cpi_handle_mut(&mut self) -> Result<crate::CpiHandleMut<'_>, ProgramError> {
        T::try_cpi_handle_mut(self.as_mut())
    }
    fn exit(&mut self) -> pinocchio::ProgramResult { (**self).exit() }
}

impl<T: crate::ToCpiHandle + ?Sized> crate::ToCpiHandle for Box<T> {
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> { self.as_ref().to_cpi_handle() }
}
impl<T: crate::ToCpiHandleMut + ?Sized> crate::ToCpiHandleMut for Box<T> {
    fn try_to_cpi_handle_mut(&mut self) -> Result<crate::CpiHandleMut<'_>, ProgramError> {
        self.as_mut().try_to_cpi_handle_mut()
    }
}
impl<T: crate::AccountRealloc> crate::AccountRealloc for Box<T> {
    fn realloc_account(&mut self, new_space: usize, payer: AccountView, zero: bool) -> pinocchio::ProgramResult {
        self.as_mut().realloc_account(new_space, payer, zero)
    }
}
impl<T: crate::AccountClose> crate::AccountClose for Box<T> {
    fn close(&mut self, destination: AccountView) -> pinocchio::ProgramResult {
        self.as_mut().close(destination)
    }
}
impl<T: crate::IdlAccountType> crate::IdlAccountType for Box<T> {
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = T::__IDL_ACCOUNT_ENTRY;
    const __IDL_TYPE_DEF: Option<&'static str> = T::__IDL_TYPE_DEF;
    const __IDL_IS_SIGNER: bool = T::__IDL_IS_SIGNER;
    const __IDL_ADDRESS: Option<&'static str> = T::__IDL_ADDRESS;
    fn __idl_account_entry() -> Option<&'static str> { T::__idl_account_entry() }
    fn __idl_type_def() -> Option<&'static str> { T::__idl_type_def() }
    fn __register_idl_deps(accounts: &mut crate::IdlEntries, types: &mut crate::IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}
impl<T: AccountInitialize> AccountInitialize for Box<T> {
    type Params<'a> = T::Params<'a>;
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        owner: &Address,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
        payer_signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        T::create_and_initialize(
            payer, account, space, owner, params, signer_seeds, payer_signer_seeds,
        )
        .map(Box::new)
    }
}
impl<T: ForeignOwnerInit> ForeignOwnerInit for Box<T> {}
impl<T: Space> Space for Box<T> { const INIT_SPACE: usize = T::INIT_SPACE; }
impl<T: Discriminator> Discriminator for Box<T> { const DISCRIMINATOR: &'static [u8] = T::DISCRIMINATOR; }
impl<C, T> AccountConstraint<Box<T>> for C
where
    C: AccountConstraint<T>,
{
    type Value = C::Value;
    fn init(account: &mut Box<T>, value: &Self::Value) -> Result<(), ProgramError> { C::init(account, value) }
    fn check(account: &Box<T>, value: &Self::Value) -> Result<(), ProgramError> { C::check(account, value) }
    fn update(account: &mut Box<T>, value: &Self::Value) -> Result<(), ProgramError> { C::update(account, value) }
    fn exit(account: &mut Box<T>, value: &Self::Value) -> Result<(), ProgramError> { C::exit(account, value) }
}
