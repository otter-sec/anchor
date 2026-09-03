//! `#[error_code]` — emits a cheap `From<E> for Error` that wraps the enum
//! discriminant as `ProgramError::Custom(code)`. The `#[msg("...")]` helper
//! is IDL-only metadata — never allocated at runtime.
//!
//! Intentionally does **not** port v1's `AnchorError` struct (heap-allocated
//! name + msg + file/line). V2 programs route error text through the IDL;
//! runtime strings duplicate that at non-trivial CU cost.

use {
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::{quote, ToTokens},
    syn::{
        parse_macro_input, Attribute, Expr, ItemEnum, Lit, Meta, MetaNameValue,
    },
};

/// Default first error code. Matches v1's `ERROR_CODE_OFFSET`.
const DEFAULT_OFFSET: u32 = 6000;

pub fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let offset = match parse_offset(args.into()) {
        Ok(Some(offset)) => offset,
        Ok(None) => DEFAULT_OFFSET,
        Err(err) => return err.to_compile_error().into(),
    };
    let mut item = parse_macro_input!(input as ItemEnum);
    let name = item.ident.clone();

    let mut errors = Vec::new();
    let mut idl_entry_pushes = Vec::new();
    for variant in item.variants.iter_mut() {
        let message = extract_msg(&variant.attrs);
        // Strip used `msg` attribute
        variant.attrs.retain(|a| !a.path().is_ident("msg"));
        if let Some((_, discr)) = &variant.discriminant {
            if parse_discrim(discr).is_none() {
                errors.push(
                    syn::Error::new_spanned(discr, "discriminant must be a u32 literal")
                        .to_compile_error(),
                );
                continue;
            }
        }
        let variant_ident = variant.ident.clone();
        let cfg_attrs = crate::cfg_attrs(&variant.attrs);
        let escaped_name = escape_json(&variant.ident.to_string());
        let suffix = match message {
            Some(message) => {
                format!(
                    ",\"name\":\"{}\",\"msg\":\"{}\"}}",
                    escaped_name,
                    escape_json(&message),
                )
            }
            None => format!(",\"name\":\"{}\"}}", escaped_name),
        };
        idl_entry_pushes.push(quote! {
            #(#cfg_attrs)*
            {
                let __code = (#name::#variant_ident as u32)
                    .checked_add(#offset)
                    .expect("error code overflowed");
                __parts.push(anchor_lang::__alloc::format!(
                    "{{\"code\":{}{}",
                    __code,
                    #suffix,
                ));
            }
        });
    }
    let idl_fn_name = quote::format_ident!(
        "__anchor_private_print_idl_errors_{}",
        name.to_string().to_lowercase()
    );

    let from_impl = quote! {
        impl From<#name> for anchor_lang::Error {
            #[inline(always)]
            fn from(e: #name) -> Self {
                // Guarenteed not to overflow in `build_idl_errors_json`
                anchor_lang::Error::Custom(e as u32 + #offset)
            }
        }
    };

    // `__idl_errors()` mirrors `__idl_accounts()` (lang-v2/derive/src/idl.rs):
    // a `pub fn` on the type returning the IDL JSON string. Lets the existing
    // `mod idl_tests { ... }` style suites parse the output with
    // `serde_json::from_str::<Vec<IdlErrorCode>>()` instead of capturing
    // stdout from the `__anchor_private_print_idl_errors_*` test fn.
    let idl_print = quote! {
        #[cfg(feature = "idl-build")]
        impl #name {
            #[doc(hidden)]
            pub fn __idl_errors() -> anchor_lang::__alloc::string::String {
                let mut __parts: anchor_lang::__alloc::vec::Vec<
                    anchor_lang::__alloc::string::String
                > = anchor_lang::__alloc::vec::Vec::new();
                #(#idl_entry_pushes)*
                let mut __payload = anchor_lang::__alloc::string::String::from("[");
                let mut __first = true;
                for __part in &__parts {
                    if !__first {
                        __payload.push(',');
                    }
                    __first = false;
                    __payload.push_str(__part);
                }
                __payload.push(']');
                __payload
            }
        }

        #[cfg(all(test, feature = "idl-build"))]
        #[test]
        fn #idl_fn_name() {
            println!("--- IDL begin errors ---");
            println!("{}", #name::__idl_errors());
            println!("--- IDL end errors ---");
        }
    };

    TokenStream::from(quote! {
        #[repr(u32)]
        #[derive(Clone, Copy)]
        #item

        #from_impl
        #idl_print
        #(#errors)*
    })
}

fn parse_offset(args: TokenStream2) -> syn::Result<Option<u32>> {
    if args.is_empty() {
        return Ok(None);
    }
    let meta: MetaNameValue = syn::parse2(args)?;
    if !meta.path.is_ident("offset") {
        return Err(syn::Error::new_spanned(
            &meta.path,
            format!(
                "unknown `#[error_code]` argument `{}`; expected `offset = N`",
                meta.path.to_token_stream()
            ),
        ));
    }
    match &meta.value {
        Expr::Lit(syn::ExprLit {
            lit: Lit::Int(i), ..
        }) => i
            .base10_parse::<u32>()
            .map(Some)
            .map_err(|_| syn::Error::new_spanned(i, "`offset` must be a u32 integer literal")),
        _ => Err(syn::Error::new_spanned(
            &meta.value,
            "`offset` must be a u32 integer literal",
        )),
    }
}

fn parse_discrim(discrim: &Expr) -> Option<u32> {
    match discrim {
        Expr::Lit(syn::ExprLit {
            lit: Lit::Int(i), ..
        }) => i.base10_parse::<u32>().ok(),
        _ => None,
    }
}

fn extract_msg(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|a| {
        if !a.path().is_ident("msg") {
            return None;
        }
        // `#[msg("text")]` parses as a list-style attribute.
        match &a.meta {
            Meta::List(list) => {
                let lit: Lit = syn::parse2(list.tokens.clone()).ok()?;
                if let Lit::Str(s) = lit {
                    Some(s.value())
                } else {
                    None
                }
            }
            _ => None,
        }
    })
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_offset_accepts_offset_assignment() {
        let offset = parse_offset(quote!(offset = 7000)).expect("offset arg should parse");

        assert_eq!(offset, Some(7000));
    }

    #[test]
    fn parse_offset_rejects_unknown_argument() {
        let err = parse_offset(quote!(unknown = 7000)).unwrap_err();

        assert!(
            err.to_string()
                .contains("unknown `#[error_code]` argument `unknown`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_offset_rejects_non_integer_literal() {
        let err = parse_offset(quote!(offset = "oops")).unwrap_err();

        assert!(
            err.to_string()
                .contains("`offset` must be a u32 integer literal"),
            "unexpected error: {err}"
        );
    }
}
