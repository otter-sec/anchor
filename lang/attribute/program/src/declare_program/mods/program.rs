use {
    super::common::get_canonical_program_id,
    heck::CamelCase,
    quote::{format_ident, quote},
};

pub fn gen_program_mod(program_name: &str) -> proc_macro2::TokenStream {
    let name = format_ident!("{}", program_name.to_camel_case());
    let id = get_canonical_program_id();
    quote! {
        /// Program definition.
        pub mod program {
            use super::*;

            /// Program type
            #[derive(Clone)]
            pub struct #name;

            impl anchor_lang::Id for #name {
                fn id() -> Pubkey {
                    #id
                }
            }
        }
    }
}
