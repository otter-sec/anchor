//! `#[pod_wrapper]` — generates a `Pod`-compatible companion type for
//! an `#[repr(u8)]` enum.
//!
//! `bytemuck::Pod` requires every bit pattern of a type to be valid, which
//! almost no Rust enum satisfies: only the declared discriminants round-trip
//! safely. Storing an enum directly in a `#[repr(C)]` Pod struct (e.g. a
//! zero-copy `#[account]`) is therefore unsound — a corrupt byte produces
//! an invalid enum value and instant UB on any pattern match.
//!
//! This attribute macro works around that. Given:
//!
//! ```ignore
//! #[pod_wrapper]
//! #[derive(Copy, Clone, PartialEq, Eq, Debug)]
//! #[repr(u8)]
//! pub enum MarketMode {
//!     Live = 0,
//!     Resolved = 1,
//! }
//! ```
//!
//! it generates a companion type `PodMarketMode`:
//!
//! ```ignore
//! #[repr(transparent)]
//! #[derive(Clone, Copy)]
//! pub struct PodMarketMode(pub u8);
//!
//! impl PodMarketMode {
//!     pub const Live: Self = Self(MarketMode::Live as u8);
//!     pub const Resolved: Self = Self(MarketMode::Resolved as u8);
//! }
//!
//! // bytemuck traits — safe because the wrapper is `#[repr(transparent)]`
//! // over `u8`, which is itself Pod. Invalid discriminants are caught at
//! // *both* equality and conversion time: any `==`, `!=`, or `.into()` on a
//! // `PodMarketMode` holding a byte that doesn't correspond to a declared
//! // variant panics. The raw-byte read via `pod.0` remains unvalidated —
//! // it's the intentional escape hatch for callers that want to inspect
//! // bytes without triggering the variant check.
//! unsafe impl anchor_lang::bytemuck::Pod for PodMarketMode {}
//! unsafe impl anchor_lang::bytemuck::Zeroable for PodMarketMode {}
//!
//! // Cross-type comparisons let existing `engine.market_mode == MarketMode::Live`
//! // keep working untouched after migrating the field from `MarketMode` to
//! // `PodMarketMode`. Both operands are validated against the declared
//! // variants, so an invalid byte can't silently bypass a negative guard
//! // like `if market_mode != MarketMode::Closed { … }`.
//! impl PartialEq<MarketMode> for PodMarketMode { /* panics on invalid */ }
//! impl PartialEq<PodMarketMode> for MarketMode { /* panics on invalid */ }
//!
//! // Round-trip conversions; `From<PodMarketMode> for MarketMode` panics on
//! // bytes that don't correspond to a declared variant.
//! impl From<MarketMode> for PodMarketMode { /* ... */ }
//! impl From<PodMarketMode> for MarketMode { /* ... */ }
//! ```
//!
//! # Requirements
//!
//! * The annotated item must be an `enum`.
//! * The enum must be declared `#[repr(u8)]` (explicit discriminant size).
//! * Every variant must be a bare unit variant (no tuple/struct payload).

use {
    proc_macro::TokenStream,
    proc_macro2::{Ident, TokenStream as TokenStream2},
    quote::{quote, quote_spanned},
    serde_json::{json, Value},
    syn::{parse_macro_input, Data, DeriveInput, Fields},
};

pub fn expand(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = input.ident.clone();
    let vis = input.vis.clone();

    let enm = match &input.data {
        Data::Enum(e) => e,
        _ => {
            return TokenStream::from(
                syn::Error::new_spanned(
                    &name,
                    "#[pod_wrapper] only supports enums — structs already have direct \
                     `#[derive(bytemuck::Pod)]` support.",
                )
                .to_compile_error(),
            );
        }
    };

    // Reject enums with payload-bearing variants: would need a tag byte
    // plus per-variant layout, which this attribute doesn't emit.
    for v in &enm.variants {
        if !matches!(v.fields, Fields::Unit) {
            return TokenStream::from(
                syn::Error::new_spanned(
                    v,
                    "#[pod_wrapper] only supports unit variants. Payload-bearing variants can't \
                     be stored in a single u8.",
                )
                .to_compile_error(),
            );
        }
    }

    // Require #[repr(u8)] so the discriminant size is explicit. Supporting
    // wider reprs later is a straightforward extension — add a discriminant
    // type parameter and rewrite the wrapper field / `as u8` casts.
    if !has_repr_u8(&input.attrs) {
        return TokenStream::from(
            syn::Error::new_spanned(
                &name,
                "#[pod_wrapper] requires `#[repr(u8)]` on the enum so the stored discriminant \
                 width is explicit.",
            )
            .to_compile_error(),
        );
    }

    let pod_name = Ident::new(&format!("Pod{}", name), name.span());
    let docs = crate::idl::extract_doc_lines(&input.attrs);
    let mut idl_type_def = serde_json::Map::new();
    idl_type_def.insert("name".into(), Value::String(pod_name.to_string()));
    if !docs.is_empty() {
        idl_type_def.insert(
            "docs".into(),
            Value::Array(docs.iter().cloned().map(Value::String).collect()),
        );
    }
    idl_type_def.insert(
        "type".into(),
        json!({
            "kind": "type",
            "alias": "u8",
        }),
    );
    let idl_type_def = syn::LitStr::new(&Value::Object(idl_type_def).to_string(), pod_name.span());

    // Per-variant associated constants keep the original variant name
    // (PascalCase), so swapping a field type from `Enum` to `PodEnum` is a
    // one-token change at each callsite — `Enum::Variant` → `PodEnum::Variant`.
    // That's a deliberate break from the usual SCREAMING_SNAKE_CASE const
    // convention in exchange for drop-in compatibility.
    let variant_consts = enm.variants.iter().map(|v| {
        let vname = &v.ident;
        quote_spanned! { vname.span() =>
            #[allow(non_upper_case_globals)]
            pub const #vname: Self = Self(#name::#vname as u8);
        }
    });

    let debug_arms = variant_match_arms(enm, &name, |_, v| {
        let vname = &v.ident;
        quote!( #name::#vname.fmt(f) )
    });

    let from_pod_arms = variant_match_arms(enm, &name, |_, v| {
        let vname = &v.ident;
        quote!( #name::#vname )
    });

    let name_str = name.to_string();
    let invalid_debug_msg = format!("{}(invalid={{}})", name_str);
    let invalid_panic_msg = format!("invalid {} discriminant: {{}}", name_str);

    let expanded: TokenStream2 = quote! {
        // Re-emit the annotated enum verbatim — attribute macros consume
        // their input, and every downstream `Enum::Variant` reference
        // still needs the original type to resolve.
        #input

        #[repr(transparent)]
        #[derive(Clone, Copy)]
        #vis struct #pod_name(pub u8);

        // SAFETY: `#pod_name` is `#[repr(transparent)]` over `u8`, so its
        // layout and alignment match `u8` exactly. Every bit pattern is a
        // valid `u8`, satisfying `Pod`'s all-bytes-initialized requirement.
        // Byte patterns that don't correspond to a declared variant are
        // surfaced at conversion time (`From<#pod_name> for #name`), not at
        // cast time — so `bytemuck`'s zero-copy cast is always sound.
        unsafe impl anchor_lang::bytemuck::Pod for #pod_name {}
        unsafe impl anchor_lang::bytemuck::Zeroable for #pod_name {}

        impl #pod_name {
            #(#variant_consts)*
        }

        impl ::core::fmt::Debug for #pod_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                match self.0 {
                    #(#debug_arms)*
                    x => write!(f, #invalid_debug_msg, x),
                }
            }
        }

        // Equality validates both sides against the declared variants before
        // comparing bytes. An invalid discriminant panics — same message as
        // `From<#pod_name> for #name` — so a negative guard like
        // `if pod != Enum::Variant { … }` can't silently execute with a
        // corrupt byte. Use the raw `pod.0` field for an unvalidated read.
        impl ::core::cmp::PartialEq for #pod_name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                let _: #name = (*self).into();
                let _: #name = (*other).into();
                self.0 == other.0
            }
        }
        impl ::core::cmp::Eq for #pod_name {}

        impl ::core::cmp::PartialEq<#name> for #pod_name {
            #[inline]
            fn eq(&self, other: &#name) -> bool {
                let _: #name = (*self).into();
                self.0 == *other as u8
            }
        }
        impl ::core::cmp::PartialEq<#pod_name> for #name {
            #[inline]
            fn eq(&self, other: &#pod_name) -> bool {
                let _: #name = (*other).into();
                *self as u8 == other.0
            }
        }

        impl ::core::convert::From<#name> for #pod_name {
            #[inline]
            fn from(v: #name) -> Self { Self(v as u8) }
        }

        impl ::core::convert::From<#pod_name> for #name {
            #[inline]
            fn from(p: #pod_name) -> Self {
                match p.0 {
                    #(#from_pod_arms)*
                    x => panic!(#invalid_panic_msg, x),
                }
            }
        }

        // `#[repr(transparent)] struct Pod{Enum}(pub u8)` — the wire shape is
        // just a byte, but it still needs a type def so downstream
        // `declare_program!` consumers can resolve `Pod{Enum}` references.
        #[cfg(feature = "idl-build")]
        impl anchor_lang::IdlAccountType for #pod_name {
            const __IDL_TYPE_DEF: Option<&'static str> = Some(#idl_type_def);

            fn __register_idl_deps(
                _accounts: &mut anchor_lang::__alloc::vec::Vec<&'static str>,
                types: &mut anchor_lang::__alloc::vec::Vec<&'static str>,
            ) {
                if let Some(t) = <Self as anchor_lang::IdlAccountType>::__IDL_TYPE_DEF {
                    types.push(t);
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Build `x if x == Name::Variant as u8 => <body>,` arms for each variant.
fn variant_match_arms<F>(enm: &syn::DataEnum, enum_name: &Ident, mut body: F) -> Vec<TokenStream2>
where
    F: FnMut(&Ident, &syn::Variant) -> TokenStream2,
{
    enm.variants
        .iter()
        .map(|v| {
            let vname = &v.ident;
            let body_ts = body(enum_name, v);
            quote! {
                x if x == #enum_name::#vname as u8 => #body_ts,
            }
        })
        .collect()
}

fn has_repr_u8(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("repr") {
            return false;
        }
        let mut found = false;
        let _ = a.parse_nested_meta(|meta| {
            if meta.path.is_ident("u8") {
                found = true;
            }
            Ok(())
        });
        found
    })
}
