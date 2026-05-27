use {
    proc_macro::TokenStream,
    proc_macro2::{Ident, TokenStream as TokenStream2},
    quote::{quote, quote_spanned, ToTokens},
    std::collections::VecDeque,
    syn::{
        parse::{Parse, ParseStream},
        parse_macro_input,
        punctuated::Punctuated,
        spanned::Spanned,
        token::Comma,
        Attribute, DeriveInput, Expr, Field, Fields, GenericArgument, PathArguments, Token, Type,
        TypeArray,
    },
};

/// Implements a [`Space`](./trait.Space.html) trait on the given
/// struct or enum.
///
/// For types that have a variable size like String and Vec, it is necessary to indicate the size by the `max_len` attribute.
/// For nested types, it is necessary to specify a size for each variable type (see example).
///
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[account]
/// #[derive(InitSpace)]
/// pub struct ExampleAccount {
///     pub data: u64,
///     #[max_len(50)]
///     pub string_one: String,
///     #[max_len(10, 5)]
///     pub nested: Vec<Vec<u8>>,
/// }
///
/// #[derive(Accounts)]
/// pub struct Initialize<'info> {
///    #[account(mut)]
///    pub payer: Signer<'info>,
///    pub system_program: Program<'info, System>,
///    #[account(init, payer = payer, space = 8 + ExampleAccount::INIT_SPACE)]
///    pub data: Account<'info, ExampleAccount>,
/// }
/// ```
#[proc_macro_derive(InitSpace, attributes(max_len))]
pub fn derive_init_space(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = input.ident.clone();

    let process_struct_fields = |fields: Punctuated<Field, Comma>| {
        let recurse = fields.into_iter().map(|f| {
            let mut max_len_args = get_max_len_args(&f.attrs);
            len_from_type(f.ty, &mut max_len_args)
        });

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
                let len = v.fields.into_iter().map(|f| {
                    let mut max_len_args = get_max_len_args(&f.attrs);
                    len_from_type(f.ty, &mut max_len_args)
                });

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
        _ => {
            return syn::Error::new(
                input.ident.span(),
                "#[derive(InitSpace)] is only supported on structs and enums",
            )
            .into_compile_error()
            .into()
        }
    };

    TokenStream::from(expanded)
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
            let path_segment = match ty_path.path.segments.last() {
                Some(seg) => seg,
                None => {
                    return syn::Error::new_spanned(ty_path, "expected a valid type path")
                        .into_compile_error()
                }
            };
            let ident = &path_segment.ident;
            let type_name = ident.to_string();
            let first_ty = get_first_ty_arg(&path_segment.arguments);

            match type_name.as_str() {
                "i8" | "u8" | "bool" => quote!(1),
                "i16" | "u16" => quote!(2),
                "i32" | "u32" | "f32" => quote!(4),
                "i64" | "u64" | "f64" => quote!(8),
                "i128" | "u128" => quote!(16),
                "String" => {
                    let max_len = get_next_arg(ident, attrs);
                    quote!((4 + #max_len))
                }
                "Pubkey" => quote!(32),
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
                _ => {
                    let ty = &ty_path.path;
                    quote!(<#ty as anchor_lang::Space>::INIT_SPACE)
                }
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
        _ => {
            let ty_type = ty.to_token_stream();
            syn::Error::new_spanned(ty_type, "Type is not supported by `#[derive(InitSpace)]`")
                .into_compile_error()
        }
    }
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
    // Parse comma-separated expressions
    let exprs = item.parse_terminated(Expr::parse, Token![,])?;
    let mut result = VecDeque::new();

    // Push them in reverse because get_next_arg() pops from the back
    for expr in exprs.into_iter().rev() {
        if !matches!(expr, Expr::Path(_) | Expr::Lit(_)) {
            return Err(syn::Error::new(
                expr.span(),
                "max_len only accepts identifiers, literals, or paths",
            ));
        }
        result.push_back(quote!((#expr as usize)));
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
