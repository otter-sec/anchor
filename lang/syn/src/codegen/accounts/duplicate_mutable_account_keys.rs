use {
    crate::{
        codegen::accounts::{generics, ParsedGenerics},
        AccountField, AccountsStruct, Field, Ty,
    },
    quote::{format_ident, quote},
};

#[derive(Clone, Copy)]
pub(super) enum KeyKind {
    Serializing,
    Loader,
}

impl KeyKind {
    pub(super) fn includes(self, field: &Field) -> bool {
        field.constraints.is_mutable()
            && !field.constraints.is_dup()
            && match self {
                Self::Serializing => matches!(
                    field.ty,
                    Ty::Account(_)
                        | Ty::LazyAccount(_)
                        | Ty::InterfaceAccount(_)
                        | Ty::Migration(_)
                ),
                Self::Loader => matches!(field.ty, Ty::AccountLoader(_)),
            }
    }

    pub(super) fn reporting_method(self) -> syn::Ident {
        match self {
            Self::Serializing => format_ident!("duplicate_mutable_account_keys"),
            Self::Loader => format_ident!("duplicate_mutable_account_loader_keys"),
        }
    }
}

/// Generates the `DuplicateMutableAccountKeys` trait implementation for an Accounts struct.
///
/// This trait is used by the duplicate mutable account validation in `try_accounts.rs`
/// to detect duplicate mutable accounts across composite (nested) account struct boundaries.
///
/// Each `#[derive(Accounts)]` struct reports the keys of its mutable accounts that
/// serialize on exit and its mutable `AccountLoader` fields separately.
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

    let methods = [KeyKind::Serializing, KeyKind::Loader].map(|kind| generate_method(accs, kind));

    quote! {
        #[automatically_derived]
        impl<#combined_generics> anchor_lang::DuplicateMutableAccountKeys for #name<#struct_generics> #where_clause {
            #(#methods)*
        }
    }
}

fn generate_method(accs: &AccountsStruct, kind: KeyKind) -> proc_macro2::TokenStream {
    let method = kind.reporting_method();
    let key_pushes = accs.fields.iter().filter_map(|af: &AccountField| match af {
        // Composite fields — delegate to inner struct's trait impl.
        // The inner struct applies its own mut/dup filters.
        AccountField::CompositeField(s) => {
            let field_name = &s.ident;
            Some(quote! {
                keys.extend(self.#field_name.#method());
            })
        }
        // Direct fields that are mut and not dup. `init` accounts are reported too: a freshly
        // created account cannot collide with an already-initialized one, but it can collide
        // with a `zero` account in an enclosing struct, since `init` leaves the account
        // program-owned and zero-filled and `zero` accepts exactly that. This impl is only
        // invoked from an enclosing struct that has composite fields, so flat structs never
        // pay for the reporting.
        AccountField::Field(f) if kind.includes(f) => {
            let field_name = &f.ident;
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
        _ => None,
    });

    quote! {
        fn #method(&self) -> Vec<anchor_lang::solana_program::pubkey::Pubkey> {
            let mut keys = Vec::new();
            #(#key_pushes)*
            keys
        }
    }
}
