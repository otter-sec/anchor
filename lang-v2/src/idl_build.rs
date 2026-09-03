//! IDL emission trait — type metadata for the IDL, parallel to
//! [`crate::AnchorAccount`] for runtime loading.
//!
//! Dispatches on the wrapper type: default returns `None` (elides
//! sysvar/signer/program/unchecked from IDL types). Data-bearing wrappers
//! (`Box<T>`, `Account<T>`, `BorshAccount<T>`, `Nested<T>`) delegate to the
//! inner type. `Slab<H, T>` is a special case: today it forwards only the
//! header `H`, because the current IDL has no faithful way to describe the
//! alignment-padded dynamic tail. User `#[account]`/`#[event]`/
//! `#[derive(IdlType)]` structs get auto-generated impls with both
//! `__IDL_ACCOUNT_ENTRY` and `__IDL_TYPE_DEF` set at macro-expansion time.
//!
//! The trait + helpers are unconditionally compiled — empty default-method
//! impls cost nothing in BPF. End-user crates opt into IDL emission via
//! their own local `idl-build` feature; the macro emissions in
//! `anchor-derive-accounts` are gated on that user-side feature.
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
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no IDL type information",
    note = "plain structs used as instruction arguments or nested fields need \
            `#[derive(IdlType)]`; account and event types get this automatically from \
            `#[account]` / `#[event]`"
)]
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

    /// Dynamic accessor for the `accounts[]` entry. Defaults to the trait
    /// const, but derive-generated impls can override it when the final IDL
    /// string depends on cfg-pruned items.
    fn __idl_account_entry() -> Option<&'static str> {
        Self::__IDL_ACCOUNT_ENTRY
    }

    /// Dynamic accessor for the `types[]` entry. Defaults to the trait const,
    /// but derive-generated impls can override it when the final IDL string
    /// depends on cfg-pruned fields or variants.
    fn __idl_type_def() -> Option<&'static str> {
        Self::__IDL_TYPE_DEF
    }

    /// Push this type's accounts/types entries (if any) and recursively
    /// register every user-defined type its fields reference. Default: no-op.
    ///
    /// Wrappers (`Box<T>`, `BorshAccount<T>`, `Nested<T>`) forward to the
    /// inner type. `Slab<H, T>` currently forwards only the header `H`;
    /// see [`crate::accounts::Slab`] for the limitation. Collection impls
    /// (`Vec<T>`, `Option<T>`, `[T; N]`, `[T]`, `&T`, `PodVec<T, N>`) forward
    /// to the element type. Primitive impls (bool, u*, i*, f*, String,
    /// Address, etc.) use the default no-op — they never appear in `types[]`.
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

// Pod scalar wrappers still lower like their native counterparts when they
// appear directly in a field type, but generic bytemuck helpers like `PodVec`
// can reference them by name. Register them as alias defs so those named
// references resolve in the final IDL.
macro_rules! impl_idl_account_type_pod_alias {
    ($(($t:path, $name:literal, $alias:literal)),* $(,)?) => {
        $(
            #[doc(hidden)]
            impl IdlAccountType for $t {
                const __IDL_TYPE_DEF: Option<&'static str> = Some(concat!(
                    "{\"name\":\"",
                    $name,
                    "\",\"type\":{\"kind\":\"type\",\"alias\":\"",
                    $alias,
                    "\"}}"
                ));

                fn __register_idl_deps(
                    _accounts: &mut alloc::vec::Vec<&'static str>,
                    types: &mut alloc::vec::Vec<&'static str>,
                ) {
                    if let Some(t) = <Self as IdlAccountType>::__IDL_TYPE_DEF {
                        types.push(t);
                    }
                }
            }
        )*
    };
}

impl_idl_account_type_pod_alias!(
    (crate::pod::PodBool, "PodBool", "bool"),
    (crate::pod::PodU16, "PodU16", "u16"),
    (crate::pod::PodU32, "PodU32", "u32"),
    (crate::pod::PodU64, "PodU64", "u64"),
    (crate::pod::PodU128, "PodU128", "u128"),
    (crate::pod::PodI16, "PodI16", "i16"),
    (crate::pod::PodI32, "PodI32", "i32"),
    (crate::pod::PodI64, "PodI64", "i64"),
    (crate::pod::PodI128, "PodI128", "i128"),
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
// It contributes a generic type definition so downstream `declare_program!`
// consumers can reconstruct the length prefix plus fixed-capacity backing
// array, then recurses into the element type.
#[doc(hidden)]
impl<T, const MAX: usize> IdlAccountType for crate::pod::PodVec<T, MAX>
where
    T: bytemuck::Pod + IdlAccountType,
{
    const __IDL_TYPE_DEF: Option<&'static str> = Some(
        "{\"name\":\"PodVec\",\"generics\":[{\"kind\":\"type\",\"name\":\"T\"},\
         {\"kind\":\"const\",\"name\":\"MAX\",\"type\":\"usize\"}],\
         \"serialization\":\"bytemuck\",\"repr\":{\"kind\":\"c\"},\
         \"type\":{\"kind\":\"struct\",\"fields\":[{\"name\":\"len\",\"type\":{\"defined\":{\"name\":\"PodU16\"}}},\
         {\"name\":\"data\",\"type\":{\"array\":[{\"generic\":\"T\"},{\"generic\":\"MAX\"}]}}]}}",
    );

    fn __register_idl_deps(
        accounts: &mut alloc::vec::Vec<&'static str>,
        types: &mut alloc::vec::Vec<&'static str>,
    ) {
        if let Some(t) = <Self as IdlAccountType>::__IDL_TYPE_DEF {
            types.push(t);
        }
        crate::pod::PodU16::__register_idl_deps(accounts, types);
        T::__register_idl_deps(accounts, types);
    }
}

#[cfg(test)]
mod tests {
    use super::IdlAccountType;

    #[test]
    fn pod_vec_registers_length_wrapper_for_native_elements() {
        let mut accounts = alloc::vec::Vec::new();
        let mut types = alloc::vec::Vec::new();

        <crate::pod::PodVec<u8, 4> as IdlAccountType>::__register_idl_deps(
            &mut accounts,
            &mut types,
        );

        assert!(
            types.iter().any(|ty| ty.contains("\"name\":\"PodVec\"")),
            "PodVec should register its own type definition: {types:?}"
        );
        assert!(
            types
                .iter()
                .any(|ty| ty.contains("\"name\":\"PodU16\"") && ty.contains("\"alias\":\"u16\"")),
            "PodVec should register its PodU16 length wrapper: {types:?}"
        );
    }

    #[test]
    fn pod_vec_registers_defined_element_wrappers() {
        let mut accounts = alloc::vec::Vec::new();
        let mut types = alloc::vec::Vec::new();

        <crate::pod::PodVec<crate::pod::PodU64, 4> as IdlAccountType>::__register_idl_deps(
            &mut accounts,
            &mut types,
        );

        assert!(
            types
                .iter()
                .any(|ty| ty.contains("\"name\":\"PodU16\"") && ty.contains("\"alias\":\"u16\"")),
            "PodVec should keep its PodU16 length helper defined: {types:?}"
        );
        assert!(
            types
                .iter()
                .any(|ty| ty.contains("\"name\":\"PodU64\"") && ty.contains("\"alias\":\"u64\"")),
            "PodVec should register named pod element wrappers: {types:?}"
        );
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

pub fn __idl_const_seeds_json<I, B>(values: I) -> alloc::string::String
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut s = alloc::string::String::from("[");
    let mut first = true;
    for value in values {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&__idl_const_seed_json(value));
    }
    s.push(']');
    s
}

/// Quote and escape an arbitrary string as a JSON string literal.
#[doc(hidden)]
pub fn __idl_json_string(value: &str) -> alloc::string::String {
    let mut out = alloc::string::String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{1F}' => out.push_str(&alloc::format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
