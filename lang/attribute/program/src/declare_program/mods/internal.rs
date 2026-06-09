use {
    super::common::{
        can_derive_clone_ty, can_derive_copy_ty, can_derive_debug_ty, can_derive_default_ty,
        convert_idl_type_to_syn_type, gen_discriminator, get_all_instruction_accounts,
        get_canonical_program_id,
    },
    anchor_lang_idl::types::{Idl, IdlInstructionAccountItem},
    anchor_syn::{
        codegen::accounts::{__client_accounts, __cpi_client_accounts},
        parser::accounts,
        AccountsStruct,
    },
    heck::CamelCase,
    quote::{format_ident, quote},
};

pub fn gen_internal_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let internal_args_mod = gen_internal_args_mod(idl);
    let internal_accounts_mod = gen_internal_accounts(idl);

    quote! {
        #[doc(hidden)]
        mod internal {
            use super::*;

            #internal_args_mod
            #internal_accounts_mod
        }
    }
}

fn gen_internal_args_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let ixs = idl.instructions.iter().map(|ix| {
        let ix_struct_name = format_ident!("{}", ix.name.to_camel_case());

        let fields = ix.args.iter().map(|arg| {
            let name = format_ident!("{}", arg.name);
            let ty = convert_idl_type_to_syn_type(&arg.ty);
            quote! { pub #name: #ty }
        });

        let ix_struct = if ix.args.is_empty() {
            quote! {
                pub struct #ix_struct_name;
            }
        } else {
            quote! {
                pub struct #ix_struct_name {
                    #(#fields),*
                }
            }
        };

        let discriminator = gen_discriminator(&ix.discriminator);
        let impl_discriminator = quote! {
            impl anchor_lang::Discriminator for #ix_struct_name {
                const DISCRIMINATOR: &'static [u8] = &#discriminator;
            }
        };

        let impl_ix_data = quote! {
            impl anchor_lang::InstructionData for #ix_struct_name {}
        };

        let program_id = get_canonical_program_id();
        let impl_owner = quote! {
            impl anchor_lang::Owner for #ix_struct_name {
                fn owner() -> Pubkey {
                    #program_id
                }
            }
        };

        // Generate conditional derives based on field types
        let copy_attr = ix
            .args
            .iter()
            .map(|field| &field.ty)
            .all(|ty| can_derive_copy_ty(ty, &idl.types))
            .then_some(quote!(#[derive(Copy)]));
        let clone_attr = ix
            .args
            .iter()
            .map(|field| &field.ty)
            .all(|ty| can_derive_clone_ty(ty, &idl.types))
            .then_some(quote!(#[derive(Clone)]));
        let debug_attr = ix
            .args
            .iter()
            .map(|field| &field.ty)
            .all(|ty| can_derive_debug_ty(ty, &idl.types))
            .then_some(quote!(#[derive(Debug)]));
        let default_attr = ix
            .args
            .iter()
            .map(|field| &field.ty)
            .all(|ty| can_derive_default_ty(ty, &idl.types))
            .then_some(quote!(#[derive(Default)]));

        quote! {
            /// Instruction argument
            #[derive(AnchorSerialize, AnchorDeserialize)]
            #copy_attr
            #clone_attr
            #debug_attr
            #default_attr
            #ix_struct

            #impl_discriminator
            #impl_ix_data
            #impl_owner
        }
    });

    quote! {
        /// An Anchor generated module containing the program's set of instructions, where each
        /// method handler in the `#[program]` mod is associated with a struct defining the input
        /// arguments to the method. These should be used directly, when one wants to serialize
        /// Anchor instruction data, for example, when specifying instructions on a
        /// client.
        pub mod args {
            use super::*;

            #(#ixs)*
        }
    }
}

fn gen_internal_accounts(idl: &Idl) -> proc_macro2::TokenStream {
    let cpi_accounts = gen_internal_accounts_common(idl, __cpi_client_accounts::generate);
    let client_accounts = gen_internal_accounts_common(idl, __client_accounts::generate);

    quote! {
        #cpi_accounts
        #client_accounts
    }
}

fn gen_internal_accounts_common(
    idl: &Idl,
    gen_accounts: impl Fn(&AccountsStruct, proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let all_ix_accs = get_all_instruction_accounts(idl);
    let accounts = all_ix_accs
        .iter()
        .map(|accs| {
            let ident = format_ident!("{}", accs.name.to_camel_case());
            let generics = if accs.accounts.is_empty() {
                quote!()
            } else {
                quote!(<'info>)
            };
            let accounts = accs.accounts.iter().map(|acc| match acc {
                IdlInstructionAccountItem::Single(acc) => {
                    let name = format_ident!("{}", acc.name);

                    let attrs = {
                        let signer = acc.signer.then_some(quote!(signer));
                        let mt = acc.writable.then_some(quote!(mut));

                        match (signer, mt) {
                            (None, None) => None,
                            (Some(s), None) => Some(quote!(#s)),
                            (None, Some(m)) => Some(quote!(#m)),
                            (Some(s), Some(m)) => Some(quote!(#s, #m)),
                        }
                    };

                    let acc_expr = if acc.optional {
                        quote! { Option<AccountInfo #generics> }
                    } else {
                        quote! { AccountInfo #generics }
                    };

                    quote! {
                        #[account(#attrs)]
                        pub #name: #acc_expr
                    }
                }
                IdlInstructionAccountItem::Composite(accs) => {
                    let name = format_ident!("{}", accs.name);
                    #[allow(
                        clippy::expect_used,
                        reason = "accounts are guaranteed to exist by prior deduplication pass"
                    )]
                    let ty_name = all_ix_accs
                        .iter()
                        .find(|a| a.accounts == accs.accounts)
                        .map(|a| format_ident!("{}", a.name.to_camel_case()))
                        .expect("Accounts must exist");

                    quote! {
                        pub #name: #ty_name #generics
                    }
                }
            });

            quote! {
                #[derive(Accounts)]
                pub struct #ident #generics {
                    #(#accounts,)*
                }
            }
        })
        .map(|accs_struct| {
            #[allow(
                clippy::expect_used,
                reason = "quote-generated tokens always produce valid syn::ItemStruct"
            )]
            let accs_struct = syn::parse2(accs_struct).expect("Failed to parse as syn::ItemStruct");
            #[allow(
                clippy::expect_used,
                reason = "quote-generated struct is always valid accounts syntax"
            )]
            let accs_struct =
                accounts::parse(&accs_struct).expect("Failed to parse accounts struct");
            gen_accounts(&accs_struct, get_canonical_program_id())
        });

    quote! { #(#accounts)* }
}
