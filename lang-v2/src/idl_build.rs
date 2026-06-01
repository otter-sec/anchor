//! IDL emission trait — type metadata for the IDL, parallel to
//! [`crate::AnchorAccount`] for runtime loading.
//!
//! Dispatches on the wrapper type: default returns `None` (elides
//! sysvar/signer/program/unchecked from IDL types). Data-bearing wrappers
//! (`Box<T>`, `Account<T>`, `BorshAccount<T>`, `Slab<H, T>`, `Nested<T>`)
//! delegate to the inner type. User `#[account]`/`#[event]`/`#[derive(IdlType)]`
//! structs get auto-generated impls with both `__IDL_ACCOUNT_ENTRY` and
//! `__IDL_TYPE_DEF` set at macro-expansion time.
//!
//! The trait + helpers are unconditionally compiled — empty default-method
//! impls cost nothing in BPF. End-user crates opt into IDL emission via
//! their own local `idl-build` feature; the macro emissions in
//! `anchor-derive-accounts-v2` are gated on that user-side feature.
//!
//! This module is exposed only for IDL generation; it is NOT part of the
//! stable API and is subject to change.

extern crate alloc;

/// Contributes (or elides) a user-defined type to the generated IDL.
///
/// **Opaque / unstable.** Do not access these consts or call
/// `__register_idl_deps` directly — they are implementation details of the
/// `anchor idl build` pipeline and will change without notice.
///
/// `__IDL_ACCOUNT_ENTRY` populates the IDL's program-level `accounts[]`
/// array (spec:137-140). `__IDL_TYPE_DEF` populates `types[]` (spec:176-188).
/// Plain `#[derive(IdlType)]` types set only the latter — they don't appear
/// in `accounts[]`. View wrappers (`Signer`, `Program<T>`, `Sysvar<T>`,
/// `UncheckedAccount`, …) leave both at `None` and surface per-wrapper
/// metadata via `__IDL_IS_SIGNER` / `__IDL_ADDRESS`.
///
/// `__register_idl_deps` handles transitive type registration: a user struct
/// pushes its own pair of strings, then recurses into field types so a
/// nested `#[derive(IdlType)] struct Inner` referenced from an `#[account]`
/// data type lands in `types[]` too.
#[diagnostic::on_unimplemented(message = "Ensure that `{Self}` has an `#[account]` attribute")]
pub trait IdlAccountType {
    /// `{"name":"X","discriminator":[…]}` for the program-level `accounts[]`.
    /// `None` for types that don't appear there (`IdlType` plain types,
    /// view wrappers, primitives, collections).
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = None;
    /// `IdlTypeDef` JSON for the program-level `types[]`. `None` for
    /// view wrappers, primitives, and collection forwarders.
    const __IDL_TYPE_DEF: Option<&'static str> = None;
    const __IDL_IS_SIGNER: bool = false;
    const __IDL_ADDRESS: Option<&'static str> = None;

    /// Push this type's accounts/types entries (if any) and recursively
    /// register every user-defined type its fields reference. Default: no-op.
    ///
    /// Wrappers (`Box<T>`, `BorshAccount<T>`, `Slab<H, T>`, `Nested<T>`)
    /// forward to the inner type; collection impls (`Vec<T>`, `Option<T>`,
    /// `[T; N]`, `[T]`, `&T`, `PodVec<T, N>`) forward to the element type.
    /// Primitive impls (bool, u*, i*, f*, String, Address, etc.) use the
    /// default no-op — they never appear in `types[]`.
    fn __register_idl_deps(
        _accounts: &mut alloc::vec::Vec<&'static str>,
        _types: &mut alloc::vec::Vec<&'static str>,
    ) {
    }
}

// ---------------------------------------------------------------------------
// Primitive + collection blanket impls
// ---------------------------------------------------------------------------
//
// All default to the no-op `__register_idl_deps` so a struct field of these
// types doesn't contribute anything to `types[]`. The collection impls
// forward to their element type so a `Vec<Inner>` field still pulls `Inner`
// into the registry.

macro_rules! impl_idl_account_type_noop {
    ($($t:ty),* $(,)?) => {
        $(
            #[doc(hidden)]
            impl IdlAccountType for $t {}
        )*
    };
}

impl_idl_account_type_noop!(
    bool,
    u8,
    u16,
    u32,
    u64,
    u128,
    i8,
    i16,
    i32,
    i64,
    i128,
    f32,
    f64,
    alloc::string::String,
    pinocchio::address::Address,
);

// Pod integer wrappers — treated the same as their native counterparts for
// IDL purposes (they map to `"u64"`, `"i32"`, etc. via `rust_type_to_idl`'s
// string-based dispatch). The blanket impl here keeps the trait resolvable
// when users reference them from nested structs.
impl_idl_account_type_noop!(
    crate::pod::PodBool,
    crate::pod::PodU16,
    crate::pod::PodU32,
    crate::pod::PodU64,
    crate::pod::PodU128,
    crate::pod::PodI16,
    crate::pod::PodI32,
    crate::pod::PodI64,
    crate::pod::PodI128,
);

#[doc(hidden)]
impl<T: IdlAccountType> IdlAccountType for alloc::vec::Vec<T> {
    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

// Borrowed slice `&[T]` — surfaces on `#[derive(IdlType)]` structs that
// carry borrowed slice fields (e.g. `MixedArgs<'a> { values: &'a [u64] }`),
// which wincode supports as a zero-copy ix arg.
#[doc(hidden)]
impl<T: IdlAccountType> IdlAccountType for [T] {
    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

// `&T` — forward to `T` so a field typed as `&'a Inner` pulls `Inner`'s
// type def into the IDL's `types[]`.
#[doc(hidden)]
impl<T: IdlAccountType + ?Sized> IdlAccountType for &T {
    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

#[doc(hidden)]
impl<T: IdlAccountType> IdlAccountType for Option<T> {
    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

#[doc(hidden)]
impl<T: IdlAccountType, const N: usize> IdlAccountType for [T; N] {
    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

// `PodVec<T, MAX>` — the zero-copy bounded-capacity analog of `Vec<T>`.
// Forward `__register_idl_deps` so a `#[account]` zero-copy type holding
// a `PodVec<Inner, 16>` still pulls `Inner` into the IDL's `types[]`.
#[doc(hidden)]
impl<T, const MAX: usize> IdlAccountType for crate::pod::PodVec<T, MAX>
where
    T: bytemuck::Pod + IdlAccountType,
{
    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

// ---------------------------------------------------------------------------
// Runtime seed-const JSON emission
// ---------------------------------------------------------------------------
//
// Used by the `Accounts` derive for opaque `seeds::program = ...`
// expressions. The expression is evaluated inside `__idl_accounts()` and
// emitted as the IDL's `{"kind":"const","value":[...]}` shape.
pub fn __idl_const_seed_json(value: impl AsRef<[u8]>) -> alloc::string::String {
    let bytes = value.as_ref();
    let mut s = alloc::string::String::from(r#"{"kind":"const","value":["#);
    let mut first = true;
    for b in bytes {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&alloc::format!("{b}"));
    }
    s.push_str("]}");
    s
}
