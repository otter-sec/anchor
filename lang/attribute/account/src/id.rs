//! Copied from solana/sdk/macro so that Anchor programs don't need to specify
//! `solana_program` as an additional crate dependency, but instead can access
//! it via `anchor_lang::declare_id`.
//!
//! Convenience macro to declare a static public key and functions to interact with it
//!
//! Input: a single literal base58 string representation of a program's id

extern crate proc_macro;

use {
    quote::{quote, ToTokens},
    syn::{
        parse::{Parse, ParseStream, Result},
        Expr, LitStr,
    },
};

fn parse_id(
    input: ParseStream,
    pubkey_type: proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    let id = if input.peek(syn::LitStr) {
        let id_literal: LitStr = input.parse()?;
        quote! { #pubkey_type::from_str_const(#id_literal) }
    } else {
        let expr: Expr = input.parse()?;
        quote! { #expr }
    };

    if !input.is_empty() {
        let stream: proc_macro2::TokenStream = input.parse()?;
        return Err(syn::Error::new_spanned(stream, "unexpected token"));
    }
    Ok(id)
}

fn id_to_tokens(
    id: &proc_macro2::TokenStream,
    pubkey_type: proc_macro2::TokenStream,
    tokens: &mut proc_macro2::TokenStream,
) {
    let event_authority_and_bump = {
        #[cfg(feature = "event-cpi")]
        quote! {
            pub const EVENT_AUTHORITY_AND_BUMP: (#pubkey_type, u8) = {
                let (address, bump) = anchor_lang::derive_program_address(&[b"__event_authority"], &ID_CONST.to_bytes());
                (#pubkey_type::new_from_array(address), bump)
            };
        }
        #[cfg(not(feature = "event-cpi"))]
        quote! {}
    };
    tokens.extend(quote! {
        /// The static program ID
        pub static ID: #pubkey_type = #id;

        /// Const version of `ID`
        pub const ID_CONST: #pubkey_type = #id;

        /// Confirms that a given pubkey is equivalent to the program ID
        pub fn check_id(id: &#pubkey_type) -> bool {
            id == &ID
        }

        /// Returns the program ID
        pub fn id() -> #pubkey_type {
            ID
        }

        /// Const version of `ID`
        pub const fn id_const() -> #pubkey_type {
            ID_CONST
        }

        #event_authority_and_bump
    });
}

pub struct Pubkey(proc_macro2::TokenStream);

impl Parse for Pubkey {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_id(
            input,
            quote! { anchor_lang::solana_program::pubkey::Pubkey },
        )
        .map(Self)
    }
}

impl ToTokens for Pubkey {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let id = &self.0;
        tokens.extend(quote! {#id})
    }
}

pub struct Id(proc_macro2::TokenStream);

impl Parse for Id {
    fn parse(input: ParseStream) -> Result<Self> {
        parse_id(
            input,
            quote! { anchor_lang::solana_program::pubkey::Pubkey },
        )
        .map(Self)
    }
}

impl ToTokens for Id {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        id_to_tokens(
            &self.0,
            quote! { anchor_lang::solana_program::pubkey::Pubkey },
            tokens,
        )
    }
}
