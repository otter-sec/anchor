use {
    crate::Program,
    heck::SnakeCase,
    quote::{quote, quote_spanned},
    syn::spanned::Spanned,
};

pub fn generate(program: &Program) -> proc_macro2::TokenStream {
    let mut accounts = std::collections::HashMap::new();
    let mut ix_spans = std::collections::HashMap::new();

    // Go through instruction accounts.
    for ix in &program.ixs {
        let anchor_ident = &ix.anchor_ident;
        // TODO: move to fn and share with accounts.rs.
        let macro_name = format!(
            "__client_accounts_{}",
            anchor_ident.to_string().to_snake_case()
        );
        ix_spans.insert(macro_name.clone(), ix.raw_method.span());
        accounts.insert(macro_name, ix.cfgs.as_slice());
    }

    // Build the tokens from all accounts
    let account_structs: Vec<proc_macro2::TokenStream> = accounts
        .iter()
        .map(|(macro_name_str, cfgs)| {
            #[allow(
                clippy::unwrap_used,
                reason = "computed from valid Rust identifier via snake_case"
            )]
            let macro_name: proc_macro2::TokenStream = macro_name_str.parse().unwrap();
            let ix_span = ix_spans
                .get(macro_name_str)
                .copied()
                .unwrap_or_else(proc_macro2::Span::call_site);
            quote_spanned! { ix_span =>
                #(#cfgs)*
                pub use crate::#macro_name::*;
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
