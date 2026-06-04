use {
    crate::{
        codegen::accounts::{bumps, constraints, generics, ParsedGenerics},
        AccountField, AccountsStruct, Ty,
    },
    quote::{quote, quote_spanned},
};

// Generates the `Accounts` trait implementation.
pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    let ParsedGenerics {
        combined_generics,
        trait_generics,
        struct_generics,
        where_clause,
    } = generics(accs);

    // Deserialization for each field
    let deser_fields: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .map(|af: &AccountField| {
            match af {
                AccountField::CompositeField(s) => {
                    let name = &s.ident;
                    let ty = &s.raw_field.ty;
                    quote! {
                        #[cfg(feature = "anchor-debug")]
                        ::anchor_lang::solana_program::log::sol_log(stringify!(#name));
                        let #name: #ty = anchor_lang::Accounts::try_accounts(__program_id, __accounts, __ix_data, &mut __bumps.#name, __reallocs)?;
                    }
                }
                AccountField::Field(f) => {
                    // `init` and `zero` accounts are special cased as they are
                    // deserialized by constraints. Here, we just take out the
                    // AccountInfo for later use at constraint validation time.
                    if is_init(af) || f.constraints.zeroed.is_some()  {
                        let name = &f.ident;
                        // Optional accounts have slightly different behavior here and
                        // we can't leverage the try_accounts implementation for zero and init.
                        if f.is_optional {
                            // Thus, this block essentially reimplements the try_accounts 
                            // behavior with optional accounts minus the deserialization.
                            let empty_behavior = if cfg!(feature = "allow-missing-optionals") {
                                quote!{ None }
                            } else {
                                quote!{ return Err(anchor_lang::error::ErrorCode::AccountNotEnoughKeys.into()); }
                            };
                            quote! {
                                let #name = if __accounts.is_empty() {
                                    #empty_behavior
                                } else if __accounts[0].key == __program_id {
                                    *__accounts = &__accounts[1..];
                                    None
                                } else {
                                    let account = &__accounts[0];
                                    *__accounts = &__accounts[1..];
                                    Some(account)
                                };
                            }
                        } else {
                            quote!{
                                if __accounts.is_empty() {
                                    return Err(anchor_lang::error::ErrorCode::AccountNotEnoughKeys.into());
                                }
                                let #name = &__accounts[0];
                                *__accounts = &__accounts[1..];
                            }
                        }
                    } else {
                        let name = f.ident.to_string();
                        let typed_name = f.typed_ident();

                        // Generate the deprecation call if it is an AccountInfo
                        let warning = if matches!(f.ty, Ty::AccountInfo) {
                            quote_spanned! { f.ty_span =>
                                ::anchor_lang::deprecated_account_info_usage();
                            }
                        } else {
                            quote! {}
                        };
                        quote! {
                            #[cfg(feature = "anchor-debug")]
                            ::anchor_lang::solana_program::log::sol_log(stringify!(#typed_name));
                            let #typed_name = anchor_lang::Accounts::try_accounts(__program_id, __accounts, __ix_data, __bumps, __reallocs)
                                .map_err(|e| e.with_account_name(#name))?;
                            #warning
                        }
                    }
                }
            }
        })
        .collect();

    let constraints = generate_constraints(accs);
    let accounts_instance = generate_accounts_instance(accs);
    let bumps_struct_name = bumps::generate_bumps_name(&accs.ident);

    let ix_de = match &accs.instruction_api {
        None => quote! {},
        Some(ix_api) => {
            let strct_inner = &ix_api;
            let field_names: Vec<proc_macro2::TokenStream> = ix_api
                .iter()
                .map(|expr: &syn::FnArg| match expr {
                    syn::FnArg::Typed(arg) => {
                        let field = &arg.pat;
                        quote! {
                            #field
                        }
                    }
                    #[allow(
                        clippy::panic,
                        reason = "invariant: ix_api args are typed fn args, validated by the \
                                  parser before codegen"
                    )]
                    _ => panic!("Invalid instruction declaration"),
                })
                .collect();
            quote! {
                let mut __ix_data = __ix_data;
                #[derive(anchor_lang::AnchorSerialize, anchor_lang::AnchorDeserialize)]
                struct __Args {
                    #strct_inner
                }
                let __Args {
                    #(#field_names),*
                } = __Args::deserialize(&mut __ix_data)
                    .map_err(|_| anchor_lang::error::ErrorCode::InstructionDidNotDeserialize)?;
            }
        }
    };

    // Generate type validation methods for instruction parameters
    let type_validation_methods = match &accs.instruction_api {
        None => {
            // generate stub methods for up to 32 possible arguments
            let stub_methods: Vec<proc_macro2::TokenStream> = (0..32)
                .map(|idx| {
                    let method_name = syn::Ident::new(
                        &format!("__anchor_validate_ix_arg_type_{}", idx),
                        proc_macro2::Span::call_site(),
                    );
                    quote! {
                        #[doc(hidden)]
                        #[inline(always)]
                        #[allow(unused)]
                        pub fn #method_name<__T>(_arg: &__T) {
                            // no type validation when #[instruction(...)] is missing
                        }
                    }
                })
                .collect();

            quote! {
                #(#stub_methods)*
            }
        }
        Some(ix_api) => {
            let declared_count = ix_api.len();

            // Generate strict validation methods for declared parameters
            let type_check_methods: Vec<proc_macro2::TokenStream> = ix_api
                .iter()
                .enumerate()
                .map(|(idx, expr)| {
                    if let syn::FnArg::Typed(arg) = expr {
                        let ty = &arg.ty;
                        let method_name = syn::Ident::new(
                            &format!("__anchor_validate_ix_arg_type_{}", idx),
                            proc_macro2::Span::call_site(),
                        );
                        quote! {
                            #[doc(hidden)]
                            #[inline(always)]
                            pub fn #method_name<__T>(_arg: &__T)
                            where
                                __T: anchor_lang::__private::IsSameType<#ty>,
                            {}
                        }
                    } else {
                        #[allow(
                            clippy::panic,
                            reason = "invariant: ix_api args are typed fn args, validated by the \
                                      parser before codegen"
                        )]
                        {
                            panic!("Invalid instruction declaration");
                        }
                    }
                })
                .collect();

            // stub methods for remaining argument positions (up to 32 total)
            let stub_methods: Vec<proc_macro2::TokenStream> = (declared_count..32)
                .map(|idx| {
                    let method_name = syn::Ident::new(
                        &format!("__anchor_validate_ix_arg_type_{}", idx),
                        proc_macro2::Span::call_site(),
                    );
                    quote! {
                        #[doc(hidden)]
                        #[inline(always)]
                        #[allow(unused)]
                        pub fn #method_name<__T>(_arg: &__T) {
                        }
                    }
                })
                .collect();

            quote! {
                #(#type_check_methods)*
                #(#stub_methods)*
            }
        }
    };

    let param_count_const = match &accs.instruction_api {
        None => quote! {
            #[automatically_derived]
            impl<#combined_generics> #name<#struct_generics> #where_clause {
                #[doc(hidden)]
                pub const __ANCHOR_IX_PARAM_COUNT: usize = 0;

                #type_validation_methods
            }
        },
        Some(ix_api) => {
            let count = ix_api.len();

            quote! {
                #[automatically_derived]
                impl<#combined_generics> #name<#struct_generics> #where_clause {
                    #[doc(hidden)]
                    pub const __ANCHOR_IX_PARAM_COUNT: usize = #count;

                    #type_validation_methods
                }
            }
        }
    };

    quote! {
        #param_count_const
        #[automatically_derived]
        impl<#combined_generics> anchor_lang::Accounts<#trait_generics, #bumps_struct_name> for #name<#struct_generics> #where_clause {
            #[inline(never)]
            fn try_accounts(
                __program_id: &anchor_lang::solana_program::pubkey::Pubkey,
                __accounts: &mut &#trait_generics [anchor_lang::solana_program::account_info::AccountInfo<#trait_generics>],
                __ix_data: &[u8],
                __bumps: &mut #bumps_struct_name,
                __reallocs: &mut std::collections::BTreeSet<anchor_lang::solana_program::pubkey::Pubkey>,
            ) -> anchor_lang::Result<Self> {
                // Deserialize instruction, if declared.
                #ix_de
                // Deserialize each account.
                #(#deser_fields)*
                // Execute accounts constraints.
                #constraints
                // Success. Return the validated accounts.
                Ok(#accounts_instance)
            }
        }
    }
}

pub fn generate_constraints(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let non_init_fields: Vec<&AccountField> =
        accs.fields.iter().filter(|af| !is_init(af)).collect();

    // Deserialization for each pda init field. This must be after
    // the initial extraction from the accounts slice and before access_checks.
    let init_fields: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .filter_map(|af| match af {
            AccountField::CompositeField(_s) => None,
            AccountField::Field(f) => match is_init(af) {
                false => None,
                true => Some(f),
            },
        })
        .map(|f| constraints::generate(f, accs))
        .collect();

    // Generate duplicate mutable account validation
    let duplicate_checks = generate_duplicate_mutable_checks(accs);

    // Constraint checks for each account fields.
    let access_checks: Vec<proc_macro2::TokenStream> = non_init_fields
        .iter()
        .map(|af: &&AccountField| match af {
            AccountField::Field(f) => constraints::generate(f, accs),
            AccountField::CompositeField(s) => constraints::generate_composite(s),
        })
        .collect();

    quote! {
        #(#init_fields)*
        #duplicate_checks
        #(#access_checks)*
    }
}

pub fn generate_accounts_instance(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    // Each field in the final deserialized accounts struct.
    let return_tys: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .map(|f: &AccountField| {
            let name = match f {
                AccountField::CompositeField(s) => &s.ident,
                AccountField::Field(f) => &f.ident,
            };
            quote! {
                #name
            }
        })
        .collect();

    quote! {
        #name {
            #(#return_tys),*
        }
    }
}

fn is_init(af: &AccountField) -> bool {
    match af {
        AccountField::CompositeField(_s) => false,
        AccountField::Field(f) => f.constraints.init.is_some(),
    }
}

// Generates duplicate mutable account validation logic
fn generate_duplicate_mutable_checks(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    // Collect all mutable account fields without `dup` constraint that serialize on exit.
    // Only types that serialize on exit are included, as duplicate mutable accounts
    // are problematic due to double serialization (the second write overwrites the first).
    // Types like UncheckedAccount, Signer, SystemAccount, AccountLoader, etc. don't serialize on exit
    let candidates: Vec<_> = accs
        .fields
        .iter()
        .filter_map(|af| match af {
            AccountField::Field(f)
                if f.constraints.is_mutable()
                    && !f.constraints.is_dup()
                    && !f.constraints.is_pure_init() =>
            {
                match &f.ty {
                    // Only include types that serialize on exit
                    crate::Ty::Account(_)
                    | crate::Ty::LazyAccount(_)
                    | crate::Ty::InterfaceAccount(_)
                    | crate::Ty::Migration(_) => Some(f),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();

    // Collect composite field idents (nested account structs)
    let composite_fields: Vec<_> = accs
        .fields
        .iter()
        .filter_map(|af| match af {
            AccountField::CompositeField(s) => Some(&s.ident),
            _ => None,
        })
        .collect();

    if candidates.is_empty() && composite_fields.is_empty() {
        return quote! {};
    }

    let mut field_keys = Vec::with_capacity(candidates.len());
    let mut field_name_strs = Vec::with_capacity(candidates.len());

    for f in candidates.iter() {
        let name = &f.ident;

        if f.is_optional {
            field_keys.push(quote! { #name.as_ref().map(|f| f.key()) });
        } else {
            field_keys.push(quote! { Some(#name.key()) });
        }

        // Use stringify! to avoid runtime allocation
        field_name_strs.push(quote! { stringify!(#name) });
    }

    // Generate code to check composite field keys
    let composite_checks: Vec<proc_macro2::TokenStream> = composite_fields
        .iter()
        .map(|composite_name| {
            quote! {
                for key in #composite_name.duplicate_mutable_account_keys() {
                    if !__mutable_accounts.insert(key) {
                        return Err(anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintDuplicateMutableAccount
                        ).with_account_name(format!("{}", key)));
                    }
                }
            }
        })
        .collect();

    quote! {
        // Duplicate mutable account validation - using HashSet
        {
            let mut __mutable_accounts = std::collections::HashSet::new();

            // Check declared mutable accounts for duplicates among themselves
            #(
                if let Some(key) = #field_keys {
                    // Check for duplicates and insert the key and account name
                    if !__mutable_accounts.insert(key) {
                        return Err(anchor_lang::error::Error::from(
                            anchor_lang::error::ErrorCode::ConstraintDuplicateMutableAccount
                        ).with_account_name(#field_name_strs));
                    }
                }
            )*

            // Check composite (nested) account struct keys for duplicates
            #(#composite_checks)*
        }
    }
}
