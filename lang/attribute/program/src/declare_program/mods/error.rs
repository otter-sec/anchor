use {
    anchor_lang_idl::types::Idl,
    heck::CamelCase,
    quote::{format_ident, quote},
};

pub fn gen_error_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let errors = idl.errors.iter().map(|e| {
        let name = format_ident!("{}", e.name);
        let code = e.code;
        let msg = e
            .msg
            .as_ref()
            .map(|msg| quote! { #[msg(#msg)] })
            .unwrap_or_default();
        quote! {
            #msg
            #name = #code,
        }
    });

    let error = if errors.len() == 0 {
        quote!()
    } else {
        let name = format_ident!("{}Error", idl.metadata.name.to_camel_case());
        quote! {
            #[anchor_lang::error_code(offset = 0)]
            pub enum #name {
                #(#errors)*
            }
        }
    };

    quote! {
        /// Program error type definitions.
        #[cfg(not(feature = "idl-build"))]
        pub mod error {
            use super::*;

            #error
        }
    }
}
