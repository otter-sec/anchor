use {super::common::gen_accounts_common, anchor_lang_idl::types::Idl, quote::quote};

pub fn gen_client_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let client_args_mod = gen_client_args_mod();
    let client_accounts_mod = gen_client_accounts_mod(idl);

    quote! {
        /// Off-chain client helpers.
        pub mod client {
            use super::*;

            #client_args_mod
            #client_accounts_mod
        }
    }
}

fn gen_client_args_mod() -> proc_macro2::TokenStream {
    quote! {
        /// Client args.
        pub mod args {
            pub use super::internal::args::*;
        }
    }
}

fn gen_client_accounts_mod(idl: &Idl) -> proc_macro2::TokenStream {
    gen_accounts_common(idl, "client")
}
