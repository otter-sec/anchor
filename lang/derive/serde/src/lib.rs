//! Defines the [`AnchorSerialize`] and [`AnchorDeserialize`] derive macros
//! These emit a `BorshSerialize`/`BorshDeserialize` implementation for the given type,
//! as well as emitting IDL type information when the `idl-build` feature is enabled.

extern crate proc_macro;

#[cfg(feature = "lazy-account")]
mod lazy;

#[cfg(feature = "lazy-account")]
use syn::spanned::Spanned;
use {
    proc_macro::TokenStream,
    proc_macro2::{Span, TokenStream as TokenStream2},
    proc_macro_crate::FoundCrate,
    quote::quote,
    syn::{parse_macro_input, DeriveInput, Ident},
};

/// Only one item-level `#[borsh]` attribute may be present, and we apply our own borsh attribute.
/// Remove any user-provided `#[borsh]` attributes to apply in our generated derive.
fn extract_borsh_attrs(input: &mut DeriveInput) -> Vec<syn::Meta> {
    input
        .attrs
        .extract_if(.., |attr| attr.path().is_ident("borsh"))
        .filter_map(|attr| {
            if let syn::Meta::List(list) = attr.meta {
                Some(list.tokens)
            } else {
                None
            }
        })
        .flat_map(|tokens| {
            syn::parse::Parser::parse2(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                tokens,
            )
            .unwrap_or_default()
        })
        .collect()
}

/// Locate any `#[borsh]` attributes on struct/enum fields,
/// which are currently unsupported with `lazy-account`.
#[cfg(feature = "lazy-account")]
fn find_field_borsh_attr(input: &DeriveInput) -> Option<&syn::Attribute> {
    match &input.data {
        syn::Data::Struct(data) => data
            .fields
            .iter()
            .flat_map(|field| field.attrs.iter())
            .find(|attr| attr.path().is_ident("borsh")),
        syn::Data::Enum(data) => data
            .variants
            .iter()
            .flat_map(|variant| variant.fields.iter())
            .flat_map(|field| field.attrs.iter())
            .find(|attr| attr.path().is_ident("borsh")),
        syn::Data::Union(data) => data
            .fields
            .named
            .iter()
            .flat_map(|field| field.attrs.iter())
            .find(|attr| attr.path().is_ident("borsh")),
    }
}

fn gen_borsh_serialize(input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as DeriveInput);
    let borsh_attrs = extract_borsh_attrs(&mut item);
    let attrs = helper_attrs("BorshSerialize", borsh_attrs);
    quote! {
        #attrs
        #item
    }
    .into()
}

#[proc_macro_derive(AnchorSerialize, attributes(borsh))]
pub fn anchor_serialize(input: TokenStream) -> TokenStream {
    #[cfg(not(feature = "idl-build"))]
    let ret = gen_borsh_serialize(input);
    #[cfg(feature = "idl-build")]
    let ret = gen_borsh_serialize(input.clone());

    #[cfg(feature = "idl-build")]
    {
        use {anchor_syn::idl::*, quote::quote, syn::Item};

        #[allow(clippy::disallowed_macros)]
        let idl_build_impl = match syn::parse(input) {
            Err(e) => return e.to_compile_error().into(),
            Ok(item) => match item {
                Item::Struct(item) => impl_idl_build_struct(&item),
                Item::Enum(item) => impl_idl_build_enum(&item),
                Item::Union(item) => impl_idl_build_union(&item),
                _ => syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "AnchorSerialize can only be derived on structs, enums, and unions",
                )
                .to_compile_error(),
            },
        };

        let ret = TokenStream2::from(ret);
        return quote! {
            #ret
            #idl_build_impl
        }
        .into();
    };

    #[cfg(not(feature = "idl-build"))]
    ret
}

fn gen_borsh_deserialize(input: TokenStream) -> TokenStream {
    let mut item = parse_macro_input!(input as DeriveInput);
    #[cfg(feature = "lazy-account")]
    if let Some(attr) = find_field_borsh_attr(&item) {
        return syn::Error::new(
            attr.span(),
            "`borsh` attributes are not currently supported with `lazy-account`",
        )
        .into_compile_error()
        .into();
    }

    let borsh_attrs = extract_borsh_attrs(&mut item);
    #[cfg(feature = "lazy-account")]
    {
        // `use_discriminant = false` is safe with `lazy-account` because it preserves
        // borsh's default sequential tag encoding (0, 1, 2, ...) which Lazy's
        // `size_of` relies on. `use_discriminant = true` would encode explicit
        // discriminant values as the tag byte, breaking the Lazy match arms.
        // Other item-level borsh attrs are not yet supported.
        let unsupported = borsh_attrs.iter().find(|attr| {
            !matches!(
                attr,
                syn::Meta::NameValue(nv)
                    if nv.path.is_ident("use_discriminant")
                        && matches!(
                            &nv.value,
                            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Bool(b), .. })
                                if !b.value
                        )
            )
        });
        if let Some(attr) = unsupported {
            return syn::Error::new(
                attr.span(),
                "only `#[borsh(use_discriminant = false)]` is supported with `lazy-account`; \
                 `use_discriminant = true` and other `borsh` attributes are not yet supported",
            )
            .into_compile_error()
            .into();
        }
    }
    let attrs = helper_attrs("BorshDeserialize", borsh_attrs);
    quote! {
        #attrs
        #item
    }
    .into()
}

/// Implements `borsh` deserialization for this structure, as well as implementing lazy
/// deserialization if the `lazy-account` feature is enabled.
/// `#[borsh(use_discriminant = false)]` is supported with `lazy-account`;
/// `use_discriminant = true` and other `#[borsh]` attributes (e.g. `skip`) are not yet
/// supported in conjunction with `lazy-account`.
///
/// ```
/// # use anchor_derive_serde::AnchorDeserialize;
/// #[derive(AnchorDeserialize)]
/// #[borsh(use_discriminant = false)]
/// pub enum Example {
///     Foo = 1,
///     Bar = 2,
/// }
/// ```
///
#[cfg_attr(feature = "lazy-account", doc = "```compile_fail")]
#[cfg_attr(
    feature = "lazy-account",
    doc = "// Will not compile with `lazy-account`"
)]
#[cfg_attr(not(feature = "lazy-account"), doc = "```")]
/// # use anchor_derive_serde::AnchorDeserialize;
/// #[derive(AnchorDeserialize)]
/// pub struct Example {
///     #[borsh(skip)]
///     x: u8,
/// }
/// ```
#[proc_macro_derive(AnchorDeserialize, attributes(borsh))]
pub fn anchor_deserialize(input: TokenStream) -> TokenStream {
    #[cfg(feature = "lazy-account")]
    {
        let deser = TokenStream2::from(gen_borsh_deserialize(input.clone()));
        let lazy = lazy::gen_lazy(input).unwrap_or_else(|e| e.to_compile_error());
        quote! {
            #deser
            #lazy
        }
        .into()
    }

    #[cfg(not(feature = "lazy-account"))]
    gen_borsh_deserialize(input)
}

fn helper_attrs(mac: &str, borsh_attrs: Vec<syn::Meta>) -> TokenStream2 {
    // We need to emit the original borsh deserialization macros on our type,
    // but derive macros can't emit other derives. To get around this, we use a hack:
    // 1. Define an `__erase` attribute macro which deletes the item it is applied to
    // 2. Emit a call to the derive, followed by a copy of the input struct with #[__erase] applied
    // 3. This results in the trait implementations being produced, but the duplicate type definition being deleted

    let mac_path = Ident::new(mac, Span::call_site());
    let anchor = match proc_macro_crate::crate_name("anchor-lang") {
        Ok(found) => found,
        Err(_) => {
            return syn::Error::new(
                Span::call_site(),
                "`anchor-derive-serde` must be used via `anchor-lang`",
            )
            .into_compile_error()
        }
    };

    let anchor_path = Ident::new(
        match &anchor {
            FoundCrate::Itself => "crate",
            FoundCrate::Name(cr) => cr.as_str(),
        },
        Span::call_site(),
    );
    let borsh_path = quote! { #anchor_path::prelude::borsh };
    let borsh_path_str = borsh_path.to_string();
    quote! {
        #[derive(#borsh_path::#mac_path)]
        // Borsh derives used in a re-export require providing the path to `borsh`
        #[borsh(crate = #borsh_path_str, #(#borsh_attrs),*)]
        #[#anchor_path::__erase]
    }
}

/// Deletes the item it is applied to. Implementation detail and not part of public API.
#[doc(hidden)]
#[proc_macro_attribute]
pub fn __erase(_: TokenStream, _: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[cfg(feature = "lazy-account")]
#[proc_macro_derive(Lazy)]
pub fn lazy(input: TokenStream) -> TokenStream {
    lazy::gen_lazy(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
