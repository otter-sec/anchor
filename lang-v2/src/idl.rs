use crate::fixed_vec::FixedVec;

#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub type IdlEntries = std::vec::Vec<&'static str>;
#[cfg(any(target_os = "solana", target_arch = "bpf"))]
pub type IdlEntries = FixedVec<&'static str, { crate::MAX_ACCOUNT_METAS }>;

/// Compile-time IDL metadata hooks. The registration surface is bounded so
/// the runtime crate does not require a heap even when IDL support is disabled.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no IDL type information",
    note = "plain structs used as instruction arguments or nested fields need \
            `#[derive(IdlType)]`; account and event types get this automatically from \
            `#[account]` / `#[event]`"
)]
pub trait IdlAccountType {
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = None;
    const __IDL_TYPE_DEF: Option<&'static str> = None;
    const __IDL_IS_SIGNER: bool = false;
    const __IDL_ADDRESS: Option<&'static str> = None;

    fn __idl_account_entry() -> Option<&'static str> {
        Self::__IDL_ACCOUNT_ENTRY
    }

    fn __idl_type_def() -> Option<&'static str> {
        Self::__IDL_TYPE_DEF
    }

    fn __register_idl_deps(_accounts: &mut IdlEntries, _types: &mut IdlEntries) {}
}

macro_rules! impl_leaf_idl_types {
    ($($ty:ty),* $(,)?) => {
        $(impl IdlAccountType for $ty {})*
    };
}

impl_leaf_idl_types!(
    (), bool, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128,
    crate::Address,
);

impl<T: IdlAccountType, const N: usize> IdlAccountType for FixedVec<T, N> {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}

#[cfg(all(feature = "alloc", any(target_os = "solana", target_arch = "bpf")))]
impl<T: IdlAccountType> IdlAccountType for alloc::vec::Vec<T> {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}

#[cfg(all(feature = "alloc", any(target_os = "solana", target_arch = "bpf")))]
impl IdlAccountType for alloc::string::String {}

impl<T: IdlAccountType + ?Sized> IdlAccountType for &T {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}
impl<T: IdlAccountType> IdlAccountType for &mut T {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}
impl<T: IdlAccountType> IdlAccountType for Option<T> {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}
impl<T: IdlAccountType, const N: usize> IdlAccountType for [T; N] {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}
impl<T: IdlAccountType> IdlAccountType for [T] {
    fn __register_idl_deps(accounts: &mut IdlEntries, types: &mut IdlEntries) {
        T::__register_idl_deps(accounts, types);
    }
}
