//! `#[derive(InitSpace)]` — ported from v1's `anchor-derive-space`.
//!
//! Computes the borsh-serialized size of a struct/enum at compile time so
//! users can write `space = 8 + MyAccount::INIT_SPACE` without hand-counting
//! bytes. For variable-length fields (`String`, `Vec<T>`), the `#[max_len(N)]`
//! helper attribute specifies the capacity reservation.

use {
    proc_macro::TokenStream,
    proc_macro2::{Ident, TokenStream as TokenStream2},
    quote::{quote, quote_spanned, ToTokens},
    std::collections::VecDeque,
    syn::{
        parse::ParseStream, parse_macro_input, punctuated::Punctuated, token::Comma, Attribute,
        DeriveInput, Expr, Field, Fields, GenericArgument, PathArguments, Type, TypeArray,
    },
};

pub fn expand(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    match crate::find_unsupported_wincode_attr(&input.attrs) {
        Ok(Some((crate::UnsupportedWincodeAttrKind::TagEncoding, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(tag_encoding = ...)]` because \
                 InitSpace assumes borsh's 1-byte enum discriminant; remove the override or \
                 compute the account size manually",
            )
            .to_compile_error()
            .into();
        }
        Ok(Some((crate::UnsupportedWincodeAttrKind::Skip, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(skip)]` because wincode \
                 overrides change the serialized layout; remove the override or compute the \
                 account size manually",
            )
            .to_compile_error()
            .into();
        }
        Ok(Some((crate::UnsupportedWincodeAttrKind::With, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(with = ...)]` because custom \
                 wincode codecs can change the serialized layout; remove the override or \
                 compute the account size manually",
            )
            .to_compile_error()
            .into();
        }
        Ok(None) => {}
        Err(err) => return err.to_compile_error().into(),
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = input.ident;

    let process_struct_fields = |fields: Punctuated<Field, Comma>| {
        let recurse = fields.into_iter().map(field_len_tokens);

        quote! {
            #[automatically_derived]
            impl #impl_generics anchor_lang::Space for #name #ty_generics #where_clause {
                const INIT_SPACE: usize = 0 #(+ #recurse)*;
            }
        }
    };

    let expanded: TokenStream2 = match input.data {
        syn::Data::Struct(strct) => match strct.fields {
            Fields::Named(named) => process_struct_fields(named.named),
            Fields::Unnamed(unnamed) => process_struct_fields(unnamed.unnamed),
            Fields::Unit => quote! {
                #[automatically_derived]
                impl #impl_generics anchor_lang::Space for #name #ty_generics #where_clause {
                    const INIT_SPACE: usize = 0;
                }
            },
        },
        syn::Data::Enum(enm) => {
            let variants = enm.variants.into_iter().map(|v| {
                let len = v.fields.into_iter().map(field_len_tokens);

                quote! {
                    0 #(+ #len)*
                }
            });

            let max = gen_max(variants);

            quote! {
                #[automatically_derived]
                impl anchor_lang::Space for #name {
                    const INIT_SPACE: usize = 1 + #max;
                }
            }
        }
        // `syn::Data` has a third variant — `Union`. Unions are exotic in
        // account data, but route the rejection through `compile_error!`
        // anyway so the user gets a targeted diagnostic on the offending
        // item instead of a proc-macro panic backtrace.
        syn::Data::Union(_) => syn::Error::new_spanned(
            &name,
            "#[derive(InitSpace)] only supports structs and enums",
        )
        .to_compile_error(),
    };

    TokenStream::from(expanded)
}

fn field_len_tokens(field: Field) -> TokenStream2 {
    match crate::find_unsupported_wincode_attr(&field.attrs) {
        Ok(Some((crate::UnsupportedWincodeAttrKind::Skip, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(skip)]` fields because \
                 wincode field overrides change the serialized layout; remove the override or \
                 compute the account size manually",
            )
            .to_compile_error();
        }
        Ok(Some((crate::UnsupportedWincodeAttrKind::With, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(with = ...)]` fields because \
                 custom wincode codecs can change the serialized layout; remove the override or \
                 compute the account size manually",
            )
            .to_compile_error();
        }
        Ok(Some((crate::UnsupportedWincodeAttrKind::TagEncoding, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(tag_encoding = ...)]` because \
                 InitSpace assumes borsh's 1-byte enum discriminant; remove the override or \
                 compute the account size manually",
            )
            .to_compile_error();
        }
        Ok(None) => {}
        Err(err) => return err.to_compile_error(),
    }

    let mut max_len_args = get_max_len_args(&field.attrs);
    len_from_type(field.ty, &mut max_len_args)
}

fn gen_max<T: Iterator<Item = TokenStream2>>(mut iter: T) -> TokenStream2 {
    if let Some(item) = iter.next() {
        let next_item = gen_max(iter);
        quote!(anchor_lang::__private::max(#item, #next_item))
    } else {
        quote!(0)
    }
}

fn len_from_type(ty: Type, attrs: &mut Option<VecDeque<TokenStream2>>) -> TokenStream2 {
    match ty {
        Type::Array(TypeArray { elem, len, .. }) => {
            let array_len = len.to_token_stream();
            let type_len = len_from_type(*elem, attrs);
            quote!((#array_len * #type_len))
        }
        Type::Path(ty_path) => {
            if let Some(type_name) = builtin_type_name(&ty_path) {
                let path_segment = ty_path
                    .path
                    .segments
                    .last()
                    .expect("syn::TypePath always has at least one segment");
                let ident = &path_segment.ident;
                let first_ty = get_first_ty_arg(&path_segment.arguments);

                match type_name {
                    "i8" | "u8" | "bool" => quote!(1),
                    "i16" | "u16" => quote!(2),
                    "i32" | "u32" | "f32" => quote!(4),
                    "i64" | "u64" | "f64" => quote!(8),
                    "i128" | "u128" => quote!(16),
                    "String" => {
                        let max_len = get_next_arg(ident, attrs);
                        quote!((4 + #max_len))
                    }
                    "Pubkey" | "Address" => quote!(32),
                    "Option" => {
                        if let Some(ty) = first_ty {
                            let type_len = len_from_type(ty, attrs);

                            quote!((1 + #type_len))
                        } else {
                            quote_spanned!(ident.span() => compile_error!("Invalid argument in Option"))
                        }
                    }
                    "Vec" => {
                        if let Some(ty) = first_ty {
                            let max_len = get_next_arg(ident, attrs);
                            let type_len = len_from_type(ty, attrs);

                            quote!((4 + #type_len * #max_len))
                        } else {
                            quote_spanned!(ident.span() => compile_error!("Invalid argument in Vec"))
                        }
                    }
                    _ => unreachable!("all builtin type names should be covered"),
                }
            } else {
                // Keep the full TypePath so `<T as Trait>::Assoc` retains
                // its qself; quoting only `.path` would emit `Trait::Assoc`.
                //
                // Arbitrary qualified paths such as `custom::Address` must not
                // match built-in shortcuts keyed on the final segment name
                // alone.
                quote!(<#ty_path as anchor_lang::Space>::INIT_SPACE)
            }
        }
        Type::Tuple(ty_tuple) => {
            let recurse = ty_tuple
                .elems
                .iter()
                .map(|t| len_from_type(t.clone(), attrs));
            quote! {
                (0 #(+ #recurse)*)
            }
        }
        // Reject unknown type shapes via `compile_error!` on the offending
        // field so the user sees a spanned diagnostic instead of a
        // proc-macro panic. The common v1→v2 mistakes are `&str` and
        // `&[T]` — call those out explicitly in the message.
        _ => syn::Error::new_spanned(
            &ty,
            "#[derive(InitSpace)] can't compute size for this type — use a fixed-size alternative \
             (e.g. `[u8; N]`), or `String` / `Vec<T>` with `#[max_len(N)]` for dynamic fields",
        )
        .to_compile_error(),
    }
}

fn builtin_type_name(ty_path: &syn::TypePath) -> Option<&'static str> {
    const PRIMITIVES_AND_ALIASES: &[(&str, &[&str])] = &[
        ("i8", &["i8"]),
        ("u8", &["u8"]),
        ("bool", &["bool"]),
        ("i16", &["i16"]),
        ("u16", &["u16"]),
        ("i32", &["i32"]),
        ("u32", &["u32"]),
        ("f32", &["f32"]),
        ("i64", &["i64"]),
        ("u64", &["u64"]),
        ("f64", &["f64"]),
        ("i128", &["i128"]),
        ("u128", &["u128"]),
        ("String", &["String"]),
        ("String", &["alloc", "string", "String"]),
        ("String", &["std", "string", "String"]),
        ("Pubkey", &["Pubkey"]),
        ("Address", &["Address"]),
        ("Option", &["Option"]),
        ("Option", &["core", "option", "Option"]),
        ("Option", &["std", "option", "Option"]),
        ("Vec", &["Vec"]),
        ("Vec", &["alloc", "vec", "Vec"]),
        ("Vec", &["std", "vec", "Vec"]),
    ];

    PRIMITIVES_AND_ALIASES
        .iter()
        .find_map(|(name, segments)| path_matches(ty_path, segments).then_some(*name))
}

fn path_matches(ty_path: &syn::TypePath, segments: &[&str]) -> bool {
    ty_path.qself.is_none()
        && ty_path.path.segments.len() == segments.len()
        && ty_path
            .path
            .segments
            .iter()
            .zip(segments.iter())
            .all(|(segment, expected)| segment.ident == *expected)
}

fn get_first_ty_arg(args: &PathArguments) -> Option<Type> {
    match args {
        PathArguments::AngleBracketed(bracket) => bracket.args.iter().find_map(|el| match el {
            GenericArgument::Type(ty) => Some(ty.to_owned()),
            _ => None,
        }),
        _ => None,
    }
}

fn parse_len_arg(item: ParseStream) -> Result<VecDeque<TokenStream2>, syn::Error> {
    let args = Punctuated::<Expr, Comma>::parse_terminated(item)?;
    let mut result = VecDeque::new();
    for arg in args {
        result.push_front(quote!((#arg as usize)));
    }

    Ok(result)
}

fn get_max_len_args(attributes: &[Attribute]) -> Option<VecDeque<TokenStream2>> {
    attributes
        .iter()
        .find(|a| a.path().is_ident("max_len"))
        .and_then(|a| a.parse_args_with(parse_len_arg).ok())
}

fn get_next_arg(ident: &Ident, args: &mut Option<VecDeque<TokenStream2>>) -> TokenStream2 {
    if let Some(arg_list) = args {
        if let Some(arg) = arg_list.pop_back() {
            quote!(#arg)
        } else {
            quote_spanned!(ident.span() => compile_error!("The number of lengths are invalid."))
        }
    } else {
        quote_spanned!(ident.span() => compile_error!("Expected max_len attribute."))
    }
}
