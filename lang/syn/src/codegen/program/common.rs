use {
    crate::IxArg,
    anyhow::Result,
    heck::CamelCase,
    quote::{quote, quote_spanned},
};

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
    let ix_name_camel = generate_ix_variant_name(name);

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

pub fn generate_ix_variant_spanned(
    name: &str,
    args: &[IxArg],
    span: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    let ix_arg_names: Vec<&syn::Ident> = args.iter().map(|arg| &arg.name).collect();
    let ix_name_camel = generate_ix_variant_name_spanned(name, span);

    if args.is_empty() {
        quote_spanned! { span =>
            #ix_name_camel
        }
    } else {
        quote_spanned! { span =>
            #ix_name_camel {
                #(#ix_arg_names),*
            }
        }
    }
}

pub fn generate_ix_variant_name(name: &str) -> syn::Ident {
    generate_ix_variant_name_spanned(name, proc_macro2::Span::call_site())
}

pub fn generate_ix_variant_name_spanned(name: &str, span: proc_macro2::Span) -> syn::Ident {
    let n = name.to_camel_case();
    syn::Ident::new(&n, span)
}
