use {crate::IxArg, heck::CamelCase, quote::quote, syn::Result};

// Namespace for calculating instruction sighash signatures for any instruction
// not affecting program state.
pub const SIGHASH_GLOBAL_NAMESPACE: &str = "global";

// We don't technically use sighash, because the input arguments aren't given.
// Rust doesn't have method overloading so no need to use the arguments.
// However, we do namespace methods in the preeimage so that we can use
// different traits with the same method name.
pub fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    let preimage = format!("{namespace}:{name}");

    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(&crate::hash::hash(preimage.as_bytes()).to_bytes()[..8]);
    sighash
}

pub fn gen_discriminator(namespace: &str, name: impl ToString) -> proc_macro2::TokenStream {
    let discriminator = sighash(namespace, name.to_string().as_str());
    #[allow(
        clippy::unwrap_used,
        reason = "debug-formatted array literal is always valid Rust token syntax"
    )]
    let ts = format!("&{discriminator:?}").parse().unwrap();
    ts
}

pub fn generate_ix_variant(name: &str, args: &[IxArg]) -> Result<proc_macro2::TokenStream> {
    let ix_arg_names: Vec<&syn::Ident> = args.iter().map(|arg| &arg.name).collect();
    let ix_name_camel = generate_ix_variant_name(name)?;

    let variant = if args.is_empty() {
        quote! {
            #ix_name_camel
        }
    } else {
        quote! {
            #ix_name_camel {
                #(#ix_arg_names),*
            }
        }
    };
    Ok(variant)
}

pub fn generate_ix_variant_name(name: &str) -> Result<syn::Ident> {
    syn::parse_str(&name.to_camel_case())
}

/// Path to the `__client_accounts_*` or `__cpi_client_accounts_*` module that
/// `#[derive(Accounts)]` emits next to the accounts struct definition.
///
/// `instructions::init::Init` with prefix `__client_accounts_` becomes
/// `crate::instructions::init::__client_accounts_init`.
pub fn generated_accounts_mod_path(anchor_path: &syn::Path, prefix: &str) -> syn::Path {
    use heck::SnakeCase;

    let mut path = anchor_path.clone();
    if let Some(last) = path.segments.last_mut() {
        last.ident = syn::Ident::new(
            &format!("{}{}", prefix, last.ident.to_string().to_snake_case()),
            last.ident.span(),
        );
    }
    path.segments.insert(
        0,
        syn::PathSegment::from(syn::Ident::new("crate", proc_macro2::Span::call_site())),
    );
    path
}

#[cfg(test)]
mod tests {
    use {super::*, quote::ToTokens};

    fn rewrite(path: &str, prefix: &str) -> String {
        #[allow(clippy::unwrap_used, reason = "test inputs are valid paths")]
        let path = syn::parse_str::<syn::Path>(path).unwrap();
        generated_accounts_mod_path(&path, prefix)
            .to_token_stream()
            .to_string()
            .replace(' ', "")
    }

    #[test]
    fn single_segment() {
        assert_eq!(
            rewrite("Initialize", "__client_accounts_"),
            "crate::__client_accounts_initialize"
        );
    }

    #[test]
    fn multi_segment() {
        assert_eq!(
            rewrite("instructions::init::Init", "__cpi_client_accounts_"),
            "crate::instructions::init::__cpi_client_accounts_init"
        );
    }

    #[test]
    fn snake_cases_multi_word_ident() {
        assert_eq!(
            rewrite("ix::UpdateCounter", "__client_accounts_"),
            "crate::ix::__client_accounts_update_counter"
        );
    }
}
