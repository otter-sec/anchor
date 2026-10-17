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

use crate::IdlAccountType;

/// Reject account discriminator collisions while building a v2 IDL.
///
/// Account entries carry a private defining-type suffix during collection so
/// repeated references to one type can be deduplicated without collapsing two
/// same-named types from different modules. The suffix is removed before the
/// public IDL is printed.
#[doc(hidden)]
pub fn validate_account_discriminator_entries(entries: &[&str]) {
    const DISC_MARKER: &str = ",\"discriminator\":[";
    const TYPE_MARKER: &str = ",\"__anchor_type\":\"";

    fn field<'a>(entry: &'a str, marker: &str, terminator: char) -> Option<&'a str> {
        let start = entry.find(marker)? + marker.len();
        let end = start + entry[start..].find(terminator)?;
        Some(&entry[start..end])
    }

    fn is_prefix(short: &str, long: &str) -> bool {
        long == short
            || long
                .strip_prefix(short)
                .is_some_and(|remainder| remainder.starts_with(','))
    }

    for (index, outer) in entries.iter().enumerate() {
        let Some(outer_disc) = field(outer, DISC_MARKER, ']') else {
            continue;
        };
        let Some(outer_type) = field(outer, TYPE_MARKER, '"') else {
            continue;
        };
        for inner in entries.iter().skip(index + 1) {
            let Some(inner_disc) = field(inner, DISC_MARKER, ']') else {
                continue;
            };
            let Some(inner_type) = field(inner, TYPE_MARKER, '"') else {
                continue;
            };
            if outer_type != inner_type
                && (is_prefix(outer_disc, inner_disc) || is_prefix(inner_disc, outer_disc))
            {
                panic!("Ambiguous discriminators for accounts `{outer_type}` and `{inner_type}`");
            }
        }
    }
}

/// Remove the private defining-type suffix from a collected account entry.
#[doc(hidden)]
pub fn strip_account_entry_identity(entry: &str) -> std::string::String {
    const TYPE_MARKER: &str = ",\"__anchor_type\":\"";
    match entry.find(TYPE_MARKER) {
        Some(index) => std::string::String::from(&entry[..index]) + "}",
        None => entry.into(),
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
    f32,
    f64,
    std::string::String,
);

// Pod scalar wrappers are referenced by name (`{"defined":{"name":"PodU64"}}`),
// both as direct field types and as `PodVec` elements, so their align-1 layout
// survives in bytemuck types. Register them as alias defs so those named
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
                    _accounts: &mut crate::IdlEntries,
                    types: &mut crate::IdlEntries,
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
impl<T: IdlAccountType> IdlAccountType for std::vec::Vec<T> {
    fn __register_idl_deps(
        accounts: &mut crate::IdlEntries,
        types: &mut crate::IdlEntries,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

#[doc(hidden)]
impl<K: IdlAccountType, V: IdlAccountType> IdlAccountType for std::collections::BTreeMap<K, V> {
    fn __register_idl_deps(
        accounts: &mut crate::IdlEntries,
        types: &mut crate::IdlEntries,
    ) {
        K::__register_idl_deps(accounts, types);
        V::__register_idl_deps(accounts, types);
    }
}

#[doc(hidden)]
impl<T: IdlAccountType> IdlAccountType for std::collections::BTreeSet<T> {
    fn __register_idl_deps(
        accounts: &mut crate::IdlEntries,
        types: &mut crate::IdlEntries,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

macro_rules! impl_idl_account_type_tuple {
    ($($ty:ident),+ $(,)?) => {
        #[doc(hidden)]
        impl<$($ty: IdlAccountType),+> IdlAccountType for ($($ty,)+) {
            fn __register_idl_deps(
                accounts: &mut crate::IdlEntries,
                types: &mut crate::IdlEntries,
            ) {
                $( $ty::__register_idl_deps(accounts, types); )+
            }
        }
    };
}

impl_idl_account_type_tuple!(A, B);
impl_idl_account_type_tuple!(A, B, C);
impl_idl_account_type_tuple!(A, B, C, D);
impl_idl_account_type_tuple!(A, B, C, D, E);
impl_idl_account_type_tuple!(A, B, C, D, E, F);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
impl_idl_account_type_tuple!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

// `PodVec<T, MAX>` — the zero-copy bounded-capacity analog of `Vec<T>`.
// It contributes a generic type definition so downstream `declare_program!`
// consumers can reconstruct the length prefix plus fixed-capacity backing
// array, then recurses into the element type.
// Pod is hand-written (`PodVecElement`), so the IDL uses `bytemuckunsafe`
// and keeps this generic `repr(C)` layout.
#[doc(hidden)]
impl<T, const MAX: usize> IdlAccountType for crate::pod::PodVec<T, MAX>
where
    T: bytemuck::Pod + IdlAccountType,
{
    const __IDL_TYPE_DEF: Option<&'static str> = Some(
        "{\"name\":\"PodVec\",\"generics\":[{\"kind\":\"type\",\"name\":\"T\"},\
         {\"kind\":\"const\",\"name\":\"MAX\",\"type\":\"usize\"}],\
         \"serialization\":\"bytemuckunsafe\",\"repr\":{\"kind\":\"c\"},\
         \"type\":{\"kind\":\"struct\",\"fields\":[{\"name\":\"len\",\"type\":{\"defined\":{\"name\":\"PodU16\"}}},\
         {\"name\":\"data\",\"type\":{\"array\":[{\"generic\":\"T\"},{\"generic\":\"MAX\"}]}}]}}",
    );

    fn __register_idl_deps(
        accounts: &mut crate::IdlEntries,
        types: &mut crate::IdlEntries,
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
        let mut accounts = crate::IdlEntries::new();
        let mut types = crate::IdlEntries::new();

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
        let mut accounts = crate::IdlEntries::new();
        let mut types = crate::IdlEntries::new();

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
pub fn __idl_const_seed_json(value: impl AsRef<[u8]>) -> std::string::String {
    let bytes = value.as_ref();
    let mut s = std::string::String::from(r#"{"kind":"const","value":["#);
    let mut first = true;
    for b in bytes {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&std::format!("{b}"));
    }
    s.push_str("]}");
    s
}

pub fn __idl_const_seeds_json<I, B>(values: I) -> std::string::String
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let mut s = std::string::String::from("[");
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
pub fn __idl_json_string(value: &str) -> std::string::String {
    let mut out = std::string::String::with_capacity(value.len() + 2);
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
            ch if ch <= '\u{1F}' => out.push_str(&std::format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
