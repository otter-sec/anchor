use {
    crate::{codegen::program::common::generated_accounts_mod_path, Program},
    quote::quote,
};

pub fn generate(program: &Program) -> proc_macro2::TokenStream {
    let mut accounts = std::collections::HashMap::new();

    // Go through instruction accounts.
    for ix in &program.ixs {
        let mod_path = generated_accounts_mod_path(&ix.anchor_path(), "__client_accounts_");
        accounts.insert(mod_path, ix.cfgs.as_slice());
    }

    // Build the tokens from all accounts
    let account_structs: Vec<proc_macro2::TokenStream> = accounts
        .iter()
        .map(|(mod_path, cfgs)| {
            quote! {
                #(#cfgs)*
                pub use #mod_path::*;
            }
        })
        .collect();

    // TODO: calculate the account size and add it as a constant field to
    //       each struct here. This is convenient for Rust clients.

    quote! {
        /// An Anchor generated module, providing a set of structs
        /// mirroring the structs deriving `Accounts`, where each field is
        /// a `Pubkey`. This is useful for specifying accounts for a client.
        pub mod accounts {
            #(#account_structs)*
        }
    }
}
