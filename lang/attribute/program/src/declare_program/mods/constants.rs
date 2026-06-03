use {
    super::common::{convert_idl_type_to_str, gen_docs},
    anchor_lang_idl::types::{Idl, IdlType},
    quote::{format_ident, quote, ToTokens},
};

pub fn gen_constants_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let constants = idl.constants.iter().map(|c| {
        let name = format_ident!("{}", c.name);
        let docs = gen_docs(&c.docs);
        #[allow(
            clippy::unwrap_used,
            reason = "compile_error! token stream is always valid syn::Type syntax"
        )]
        let ty = convert_idl_type_to_str(&c.ty, true)
            .and_then(|s| {
                syn::parse_str::<syn::Type>(&s)
                    .map_err(|err| syn::Error::new(proc_macro2::Span::call_site(), err.to_string()))
            })
            .unwrap_or_else(|err| syn::parse2(err.into_compile_error()).unwrap());
        #[allow(
            clippy::unwrap_used,
            reason = "IDL constant values are valid Rust expressions by construction"
        )]
        let val = syn::parse_str::<syn::Expr>(&c.value)
            .unwrap()
            .to_token_stream();
        let val = match &c.ty {
            IdlType::Bytes => quote! { &#val },
            IdlType::Pubkey => quote!(Pubkey::from_str_const(stringify!(#val))),
            _ => val,
        };

        quote! {
            #docs
            pub const #name: #ty = #val;
        }
    });

    quote! {
        /// Program constants.
        pub mod constants {
            use super::*;

            #(#constants)*
        }
    }
}
