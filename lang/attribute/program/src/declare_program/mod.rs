mod common;
mod mods;

use {
    anchor_lang_idl::{convert::convert_idl, types::Idl},
    anyhow::anyhow,
    common::gen_docs,
    mods::{
        accounts::gen_accounts_mod, client::gen_client_mod, constants::gen_constants_mod,
        cpi::gen_cpi_mod, error::gen_error_mod, events::gen_events_mod, internal::gen_internal_mod,
        parsers::gen_parsers_mod, program::gen_program_mod, types::gen_types_mod,
    },
    quote::{quote, ToTokens},
    std::{env, fs, path::PathBuf},
    syn::parse::{Parse, ParseStream},
};

pub struct DeclareProgram {
    name: syn::Ident,
    idl: Idl,
}

impl Parse for DeclareProgram {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let idl = get_idl(&name).map_err(|e| syn::Error::new(name.span(), e))?;
        Ok(Self { name, idl })
    }
}

impl ToTokens for DeclareProgram {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let program = gen_program(&self.idl, &self.name);
        tokens.extend(program)
    }
}

fn get_idl(name: &syn::Ident) -> anyhow::Result<Idl> {
    env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .map_err(|e| anyhow!("Failed to get environment variable `CARGO_MANIFEST_DIR`: {e}"))?
        .ancestors()
        .find_map(|ancestor| {
            let idl_dir = ancestor.join("idls");
            idl_dir.exists().then_some(idl_dir)
        })
        .ok_or_else(|| anyhow!("`idls` directory not found"))
        .map(|idl_dir| idl_dir.join(name.to_string()).with_extension("json"))
        .map(fs::read)?
        .map_err(|e| anyhow!("Failed to read IDL `{name}`: {e}"))
        .map(|buf| convert_idl(&buf))?
}

fn gen_program(idl: &Idl, name: &syn::Ident) -> proc_macro2::TokenStream {
    let docs = gen_program_docs(idl);
    let id = gen_id(idl);
    let program_mod = gen_program_mod(&idl.metadata.name);

    // Defined
    let constants_mod = gen_constants_mod(idl);
    let accounts_mod = gen_accounts_mod(idl);
    let events_mod = gen_events_mod(idl);
    let types_mod = gen_types_mod(idl);
    let error_mod = gen_error_mod(idl);

    // Clients
    let cpi_mod = gen_cpi_mod(idl);
    let client_mod = gen_client_mod(idl);
    let internal_mod = gen_internal_mod(idl);

    // Utils
    let parsers_mod = gen_parsers_mod(idl);

    quote! {
        #docs
        pub mod #name {
            #[cfg(any(target_os = "solana", feature = "idl-build"))]
            use ::anchor_lang;
            #[cfg(all(not(target_os = "solana"), not(feature = "idl-build")))]
            use super::anchor_lang;

            use anchor_lang::prelude::*;

            /// IDL-generated re-exports.
            /// These are placed in a separate module to avoid name collisions with the prelude glob import
            mod __defined {
                pub use super::accounts::*;
                pub use super::events::*;
                pub use super::types::*;
            }

            #id
            #program_mod

            #constants_mod
            #accounts_mod
            #events_mod
            #types_mod
            #error_mod

            #cpi_mod
            #client_mod
            #internal_mod

            #parsers_mod
        }
    }
}

fn gen_program_docs(idl: &Idl) -> proc_macro2::TokenStream {
    let docs: &[String] = &[
        format!(
            "Generated external program declaration of program `{}`.",
            idl.metadata.name
        ),
        String::default(),
    ];
    let docs = [docs, &idl.docs].concat();
    gen_docs(&docs)
}

fn gen_id(idl: &Idl) -> proc_macro2::TokenStream {
    let address = &idl.address;
    let doc = format!("Program ID of program `{}`.", idl.metadata.name);

    quote! {
        #[doc = #doc]
        pub static ID: Pubkey = __ID;

        /// Const version of `ID`
        pub const ID_CONST: Pubkey = __ID_CONST;

        /// The name is intentionally prefixed with `__` in order to reduce to possibility of name
        /// clashes with the crate's `ID`.
        static __ID: Pubkey = Pubkey::from_str_const(#address);
        const __ID_CONST : Pubkey = Pubkey::from_str_const(#address);
    }
}
