use {super::common::convert_idl_type_def_to_ts, anchor_lang_idl::types::Idl, quote::quote};

pub fn gen_types_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let types = idl
        .types
        .iter()
        .filter(|ty| {
            // Skip accounts and events
            !(idl.accounts.iter().any(|acc| acc.name == ty.name)
                || idl.events.iter().any(|ev| ev.name == ty.name))
        })
        .map(|ty| convert_idl_type_def_to_ts(ty, &idl.types));

    quote! {
        /// Program type definitions.
        ///
        /// Note that account and event type definitions are not included in this module, as they
        /// have their own dedicated modules.
        pub mod types {
            use super::*;

            #(#types)*
        }
    }
}
