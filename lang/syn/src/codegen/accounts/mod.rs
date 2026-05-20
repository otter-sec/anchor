use {
    crate::AccountsStruct,
    quote::quote,
    std::iter,
    syn::{
        punctuated::Punctuated, ConstParam, GenericParam, LifetimeParam, PredicateLifetime, Token,
        TypeParam, WhereClause, WherePredicate,
    },
};

pub mod __client_accounts;
pub mod __cpi_client_accounts;
mod bumps;
mod constraints;
mod duplicate_mutable_account_keys;
mod exit;
mod to_account_infos;
mod to_account_metas;
mod try_accounts;

pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let impl_try_accounts = try_accounts::generate(accs);
    let impl_to_account_infos = to_account_infos::generate(accs);
    let impl_to_account_metas = to_account_metas::generate(accs);
    let impl_exit = exit::generate(accs);
    let impl_dup_mutable_keys = duplicate_mutable_account_keys::generate(accs);
    let bumps_struct = bumps::generate(accs);

    let program_id = quote! {
        // In a doctest the ID will be in the current scope, not the crate root
        {
            #[cfg(not(doctest))]
            { crate::ID }
            #[cfg(doctest)]
            { ID }
        }
    };

    let __client_accounts_mod = __client_accounts::generate(accs, program_id.clone());
    let __cpi_client_accounts_mod = __cpi_client_accounts::generate(accs, program_id);

    let ret = quote! {
        #impl_try_accounts
        #impl_to_account_infos
        #impl_to_account_metas
        #impl_exit
        #impl_dup_mutable_keys
        #bumps_struct

        #__client_accounts_mod
        #__cpi_client_accounts_mod
    };

    #[cfg(feature = "idl-build")]
    {
        let idl_build_impl = crate::idl::gen_idl_build_impl_accounts_struct(accs);
        return quote! {
            #ret
            #idl_build_impl
        };
    }

    #[allow(unreachable_code)]
    ret
}

fn generics(accs: &AccountsStruct) -> ParsedGenerics {
    #[allow(
        clippy::expect_used,
        reason = "'info is a hardcoded valid lifetime string"
    )]
    let trait_lifetime = accs
        .generics
        .lifetimes()
        .next()
        .cloned()
        .unwrap_or_else(|| syn::parse_str("'info").expect("Could not parse lifetime"));

    let mut where_clause = accs.generics.where_clause.clone().unwrap_or(WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });
    for lifetime in accs.generics.lifetimes().map(|def| &def.lifetime) {
        where_clause
            .predicates
            .push(WherePredicate::Lifetime(PredicateLifetime {
                lifetime: lifetime.clone(),
                colon_token: Default::default(),
                bounds: iter::once(trait_lifetime.lifetime.clone()).collect(),
            }))
    }
    let trait_lifetime = GenericParam::Lifetime(trait_lifetime);

    ParsedGenerics {
        combined_generics: if accs.generics.lifetimes().next().is_some() {
            accs.generics.params.clone()
        } else {
            iter::once(trait_lifetime.clone())
                .chain(accs.generics.params.clone())
                .collect()
        },
        trait_generics: iter::once(trait_lifetime).collect(),
        struct_generics: accs
            .generics
            .params
            .clone()
            .into_iter()
            .map(|param: GenericParam| match param {
                GenericParam::Const(ConstParam { ident, .. })
                | GenericParam::Type(TypeParam { ident, .. }) => GenericParam::Type(TypeParam {
                    attrs: vec![],
                    ident,
                    colon_token: None,
                    bounds: Default::default(),
                    eq_token: None,
                    default: None,
                }),
                GenericParam::Lifetime(LifetimeParam { lifetime, .. }) => {
                    GenericParam::Lifetime(LifetimeParam {
                        attrs: vec![],
                        lifetime,
                        colon_token: None,
                        bounds: Default::default(),
                    })
                }
            })
            .collect(),
        where_clause,
    }
}

struct ParsedGenerics {
    pub combined_generics: Punctuated<GenericParam, Token![,]>,
    pub trait_generics: Punctuated<GenericParam, Token![,]>,
    pub struct_generics: Punctuated<GenericParam, Token![,]>,
    pub where_clause: WhereClause,
}
