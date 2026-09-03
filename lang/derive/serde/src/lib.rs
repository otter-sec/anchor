//! Defines the [`AnchorSerialize`] and [`AnchorDeserialize`] derive macros
//! These emit a `BorshSerialize`/`BorshDeserialize` implementation for the given type,
//! as well as emitting IDL type information when the `idl-build` feature is enabled.

extern crate proc_macro;

#[cfg(feature = "lazy-account")]
mod lazy;

use {
    proc_macro::TokenStream,
    proc_macro2::{Span, TokenStream as TokenStream2},
    proc_macro_crate::FoundCrate,
    quote::quote,
    syn::Ident,
};

fn gen_borsh_serialize(input: TokenStream) -> TokenStream2 {
    let input = TokenStream2::from(input);
    let attrs = helper_attrs("BorshSerialize");
    quote! {
        #attrs
        #input
    }
}

#[proc_macro_derive(AnchorSerialize)]
pub fn anchor_serialize(input: TokenStream) -> TokenStream {
    #[cfg(not(feature = "idl-build"))]
    let ret = gen_borsh_serialize(input);
    #[cfg(feature = "idl-build")]
    let ret = gen_borsh_serialize(input.clone());

    #[cfg(feature = "idl-build")]
    {
        use {anchor_syn::idl::*, quote::quote, syn::Item};

        let idl_build_impl = match syn::parse(input).unwrap() {
            Item::Struct(item) => impl_idl_build_struct(&item),
            Item::Enum(item) => impl_idl_build_enum(&item),
            Item::Union(item) => impl_idl_build_union(&item),
            // Derive macros can only be defined on structs, enums, and unions.
            _ => unreachable!(),
        };

        return TokenStream::from(quote! {
            #ret
            #idl_build_impl
        });
    };

    #[allow(unreachable_code)]
    TokenStream::from(ret)
}

fn gen_borsh_deserialize(input: TokenStream) -> TokenStream2 {
    let input = TokenStream2::from(input);
    let attrs = helper_attrs("BorshDeserialize");
    quote! {
        #attrs
        #input
    }
}

#[proc_macro_derive(AnchorDeserialize)]
pub fn borsh_deserialize(input: TokenStream) -> TokenStream {
    #[cfg(feature = "lazy-account")]
    {
        let deser = gen_borsh_deserialize(input.clone());
        let lazy = lazy::gen_lazy(input).unwrap_or_else(|e| e.to_compile_error());
        quote::quote! {
            #deser
            #lazy
        }
        .into()
    }
    #[cfg(not(feature = "lazy-account"))]
    gen_borsh_deserialize(input).into()
}

fn helper_attrs(mac: &str) -> TokenStream2 {
    // We need to emit the original borsh deserialization macros on our type,
    // but derive macros can't emit other derives. To get around this, we use a hack:
    // 1. Define an `__erase` attribute macro which deletes the item it is applied to
    // 2. Emit a call to the derive, followed by a copy of the input struct with #[__erase] applied
    // 3. This results in the trait implementations being produced, but the duplicate type definition being deleted

    let mac_path = Ident::new(mac, Span::call_site());
    let anchor = proc_macro_crate::crate_name("anchor-lang-v1")
        .expect("`anchor-derive-serde` must be used via `anchor-lang-v1`");

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
        #[borsh(crate = #borsh_path_str)]
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
