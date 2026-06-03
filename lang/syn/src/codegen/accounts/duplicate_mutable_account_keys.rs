use {
    crate::{
        codegen::accounts::{generics, ParsedGenerics},
        AccountField, AccountsStruct,
    },
    quote::quote,
};

/// Generates the `DuplicateMutableAccountKeys` trait implementation for an Accounts struct.
///
/// This trait is used by the duplicate mutable account validation in `try_accounts.rs`
/// to detect duplicate mutable accounts across composite (nested) account struct boundaries.
///
/// Each `#[derive(Accounts)]` struct implements this trait to report the pubkeys of its
/// mutable accounts that serialize on exit. The outer struct's duplicate check collects
/// keys from both its direct fields and composite fields (via this trait) into a single
/// HashSet to detect duplicates.
///
/// ## Example
///
/// Given:
/// ```ignore
/// #[derive(Accounts)]
/// pub struct Wrapper<'info> {
///     #[account(mut)]
///     pub counter: Account<'info, Counter>,
///     pub readonly: Account<'info, Data>,       // excluded: not mut
/// }
///
/// #[derive(Accounts)]
/// pub struct Outer<'info> {
///     #[account(mut)]
///     pub direct: Account<'info, Counter>,
///     pub authority: Signer<'info>,              // excluded: not a serializing type
///     pub wrapper: Wrapper<'info>,
/// }
/// ```
///
/// Generates:
/// ```ignore
/// impl DuplicateMutableAccountKeys for Wrapper<'_> {
///     fn duplicate_mutable_account_keys(&self) -> Vec<Pubkey> {
///         let mut keys = Vec::new();
///         keys.push(self.counter.key());   // mut Account → included
///         // readonly accounts excluded
///         keys
///     }
/// }
///
/// impl DuplicateMutableAccountKeys for Outer<'_> {
///     fn duplicate_mutable_account_keys(&self) -> Vec<Pubkey> {
///         let mut keys = Vec::new();
///         keys.push(self.direct.key());                                // direct mut field
///         // authority excluded: Signer doesn't serialize on exit
///         keys.extend(self.wrapper.duplicate_mutable_account_keys());  // delegate to inner
///         keys
///     }
/// }
/// ```
pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    let ParsedGenerics {
        combined_generics,
        trait_generics: _,
        struct_generics,
        where_clause,
    } = generics(accs);

    let key_pushes: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .filter_map(|af: &AccountField| match af {
            // Composite fields — delegate to inner struct's trait impl.
            // The inner struct applies its own mut/dup/init filters.
            AccountField::CompositeField(s) => {
                let field_name = &s.ident;
                Some(quote! {
                    keys.extend(self.#field_name.duplicate_mutable_account_keys());
                })
            }
            // Direct fields that are mut, not dup, and not pure init (init_if_needed is included).
            // Pure-init accounts are always freshly created so they cannot collide
            // with an existing mutable account. `init_if_needed` accounts, however,
            // may already be initialized and therefore must participate in the
            // duplicate-mutable-account check.
            AccountField::Field(f)
                if f.constraints.is_mutable()
                    && !f.constraints.is_dup()
                    && !f.constraints.is_pure_init() =>
            {
                // Only types that serialize on exit (not Signer, Program, etc.).
                match &f.ty {
                    crate::Ty::Account(_)
                    | crate::Ty::LazyAccount(_)
                    | crate::Ty::InterfaceAccount(_)
                    | crate::Ty::Migration(_) => {
                        let field_name = &f.ident;
                        // Optional accounts need an if-let guard.
                        if f.is_optional {
                            Some(quote! {
                                if let Some(ref account) = self.#field_name {
                                    keys.push(account.key());
                                }
                            })
                        } else {
                            Some(quote! {
                                keys.push(self.#field_name.key());
                            })
                        }
                    }
                    // Non-serializing types (e.g. AccountLoader) — skip.
                    _ => None,
                }
            }
            // Everything else (readonly, Signers, Programs, failed guard).
            _ => None,
        })
        .collect();

    quote! {
        #[automatically_derived]
        impl<#combined_generics> anchor_lang::DuplicateMutableAccountKeys for #name<#struct_generics> #where_clause {
            fn duplicate_mutable_account_keys(&self) -> Vec<anchor_lang::solana_program::pubkey::Pubkey> {
                let mut keys = Vec::new();
                #(#key_pushes)*
                keys
            }
        }
    }
}
