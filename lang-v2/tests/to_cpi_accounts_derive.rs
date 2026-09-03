use anchor_lang::{
    prelude::*,
    testing::{AccountBuffer, MIN_ACCOUNT_BUF},
};
use core::marker::PhantomData;

const ID: Address = Address::new_from_array([9; 32]);
const AUTHORITY_SIGNER: bool = true;

#[derive(ToCpiAccounts)]
struct InnerCpi<'a> {
    inner_readonly: CpiHandle<'a>,
    inner_writable: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
struct EmptyCpi<'a> {
    _phantom: PhantomData<&'a ()>,
}

#[derive(ToCpiAccounts)]
struct DuplicateReadonlyCpi<'a> {
    #[account_meta(duplicate_readonly)]
    readonly: CpiHandle<'a>,
    #[account_meta(duplicate_readonly)]
    writable: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
struct ManualCpi<'a> {
    readonly: CpiHandle<'a>,
    writable: CpiHandleMut<'a>,
    #[signer(self.authority_signer)]
    authority: CpiHandle<'a>,
    #[account_meta(skip)]
    authority_signer: bool,
    #[nested]
    inner: InnerCpi<'a>,
    optional_readonly: Option<CpiHandle<'a>>,
    #[signer]
    optional_writable_signer: Option<CpiHandleMut<'a>>,
    absent: Option<CpiHandleMut<'a>>,
}

fn account(
    address: [u8; 32],
    signer: bool,
    writable: bool,
) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
    let buffer = AccountBuffer::new();
    buffer.init(address, [1; 32], 8, signer, writable, false);
    buffer
}

#[test]
fn derive_to_cpi_accounts_emits_metas_and_erased_handles() {
    let readonly_buffer = account([1; 32], false, false);
    let writable_buffer = account([2; 32], false, true);
    let authority_buffer = account([3; 32], true, false);
    let optional_buffer = account([4; 32], false, false);
    let optional_writable_buffer = account([5; 32], true, true);
    let inner_readonly_buffer = account([6; 32], false, false);
    let inner_writable_buffer = account([7; 32], false, true);

    let readonly_view = unsafe { readonly_buffer.view() };
    let mut writable_view = unsafe { writable_buffer.view() };
    let authority_view = unsafe { authority_buffer.view() };
    let optional_view = unsafe { optional_buffer.view() };
    let mut optional_writable_view = unsafe { optional_writable_buffer.view() };
    let inner_readonly_view = unsafe { inner_readonly_buffer.view() };
    let mut inner_writable_view = unsafe { inner_writable_buffer.view() };

    let accounts = ManualCpi {
        readonly: readonly_view.to_cpi_handle(),
        writable: writable_view.to_cpi_handle_mut(),
        authority: authority_view.to_cpi_handle(),
        authority_signer: AUTHORITY_SIGNER,
        inner: InnerCpi {
            inner_readonly: inner_readonly_view.to_cpi_handle(),
            inner_writable: inner_writable_view.to_cpi_handle_mut(),
        },
        optional_readonly: Some(optional_view.to_cpi_handle()),
        optional_writable_signer: Some(optional_writable_view.to_cpi_handle_mut()),
        absent: None,
    };

    let metas = accounts.to_instruction_accounts();
    assert_eq!(metas.len(), 8);
    assert_eq!(*metas[0].address, Address::new_from_array([1; 32]));
    assert!(!metas[0].is_writable);
    assert!(!metas[0].is_signer);
    assert_eq!(*metas[1].address, Address::new_from_array([2; 32]));
    assert!(metas[1].is_writable);
    assert!(!metas[1].is_signer);
    assert_eq!(*metas[2].address, Address::new_from_array([3; 32]));
    assert!(!metas[2].is_writable);
    assert!(metas[2].is_signer);
    assert_eq!(*metas[3].address, Address::new_from_array([6; 32]));
    assert!(!metas[3].is_writable);
    assert!(!metas[3].is_signer);
    assert_eq!(*metas[4].address, Address::new_from_array([7; 32]));
    assert!(metas[4].is_writable);
    assert!(!metas[4].is_signer);
    assert_eq!(*metas[5].address, Address::new_from_array([4; 32]));
    assert!(!metas[5].is_writable);
    assert!(!metas[5].is_signer);
    assert_eq!(*metas[6].address, Address::new_from_array([5; 32]));
    assert!(metas[6].is_writable);
    assert!(metas[6].is_signer);
    assert_eq!(*metas[7].address, ID);
    assert!(!metas[7].is_writable);
    assert!(!metas[7].is_signer);

    let handles = accounts.to_cpi_handles();
    assert_eq!(handles.len(), 7);
    assert!(!handles[0].is_writable());
    assert!(handles[1].is_writable());
    assert!(!handles[2].is_writable());
    assert!(!handles[3].is_writable());
    assert!(handles[4].is_writable());
    assert!(!handles[5].is_writable());
    assert!(handles[6].is_writable());

    let flags = accounts.optional_account_sentinel_flags();
    assert_eq!(flags, vec![false, false, false, false, false, false, false, true]);
}

#[test]
fn derive_to_cpi_accounts_accepts_phantom_only_empty_structs() {
    let accounts = EmptyCpi {
        _phantom: PhantomData,
    };

    assert!(accounts.to_instruction_accounts().is_empty());
    assert!(accounts.to_cpi_handles().is_empty());
    assert!(accounts.optional_account_sentinel_flags().is_empty());
}

#[test]
fn derive_to_cpi_accounts_duplicate_readonly_erases_handle_mut() {
    let readonly_buffer = account([1; 32], false, false);
    let writable_buffer = account([2; 32], false, true);
    let readonly_view = unsafe { readonly_buffer.view() };
    let mut writable_view = unsafe { writable_buffer.view() };

    let accounts = DuplicateReadonlyCpi {
        readonly: readonly_view.to_cpi_handle(),
        writable: writable_view.to_cpi_handle_mut(),
    };

    let metas = accounts.to_instruction_accounts();
    assert_eq!(metas.len(), 4);
    assert_eq!(*metas[0].address, Address::new_from_array([1; 32]));
    assert!(!metas[0].is_writable);
    assert_eq!(*metas[1].address, Address::new_from_array([1; 32]));
    assert!(!metas[1].is_writable);
    assert_eq!(*metas[2].address, Address::new_from_array([2; 32]));
    assert!(!metas[2].is_writable);
    assert_eq!(*metas[3].address, Address::new_from_array([2; 32]));
    assert!(metas[3].is_writable);

    let handles = accounts.to_cpi_handles();
    assert_eq!(handles.len(), 4);
    assert!(!handles[0].is_writable());
    assert!(!handles[1].is_writable());
    assert!(!handles[2].is_writable());
    assert!(handles[3].is_writable());
    assert_eq!(*handles[0].address(), Address::new_from_array([1; 32]));
    assert_eq!(*handles[2].address(), Address::new_from_array([2; 32]));
    assert_eq!(*handles[3].address(), Address::new_from_array([2; 32]));
}
