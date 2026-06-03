use {
    crate::{
        accounts_codegen::constraints::OptionalCheckScope,
        codegen::accounts::{generics, ParsedGenerics},
        AccountField, AccountsStruct, Ty,
    },
    quote::quote,
};

// Generates the `Exit` trait implementation.
pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    let ParsedGenerics {
        combined_generics,
        trait_generics,
        struct_generics,
        where_clause,
    } = generics(accs);

    let on_save: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .map(|af: &AccountField| match af {
            AccountField::CompositeField(s) => {
                let name = &s.ident;
                let name_str = name.to_string();
                quote! {
                    anchor_lang::AccountsExit::exit(&self.#name, program_id)
                        .map_err(|e| e.with_account_name(#name_str))?;
                }
            }
            AccountField::Field(f) => {
                let ident = &f.ident;
                let name_str = ident.to_string();
                if f.constraints.is_close() {
                    #[allow(
                        clippy::unwrap_used,
                        reason = "is_close() guarantees close constraint is Some"
                    )]
                    let close_target = &f.constraints.close.as_ref().unwrap().sol_dest;
                    let close_target_optional_check =
                        OptionalCheckScope::new(accs).generate_check(close_target);

                    quote! {
                        {
                            let #close_target = &self.#close_target;
                            #close_target_optional_check
                            anchor_lang::AccountsClose::close(
                                &self.#ident,
                                #close_target.to_account_info(),
                            ).map_err(|e| e.with_account_name(#name_str))?;
                        }
                    }
                } else {
                    match f.constraints.is_mutable() {
                        false => quote! {},
                        true => match &f.ty {
                            // `LazyAccount` is special because it has a custom `exit` method.
                            Ty::LazyAccount(_) => quote! {
                                self.#ident.exit(program_id)
                                    .map_err(|e| e.with_account_name(#name_str))?;
                            },
                            _ => quote! {
                                anchor_lang::AccountsExit::exit(&self.#ident, program_id)
                                    .map_err(|e| e.with_account_name(#name_str))?;
                            },
                        },
                    }
                }
            }
        })
        .collect();
    quote! {
        #[automatically_derived]
        impl<#combined_generics> anchor_lang::AccountsExit<#trait_generics> for #name<#struct_generics> #where_clause{
            fn exit(&self, program_id: &anchor_lang::solana_program::pubkey::Pubkey) -> anchor_lang::Result<()> {
                #(#on_save)*
                Ok(())
            }
        }
    }
}
