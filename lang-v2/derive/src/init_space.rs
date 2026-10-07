//! `#[derive(InitSpace)]` — ported from v1's `anchor-derive-space`.
//!
//! Computes the borsh-serialized size of a struct/enum at compile time so
//! users can write `space = 8 + MyAccount::INIT_SPACE` without hand-counting
//! bytes. For variable-length fields (`String`, `Vec<T>`), the `#[max_len(N)]`
//! helper attribute specifies the capacity reservation.

use {
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::{format_ident, quote},
    syn::{
        parse::ParseStream, parse_macro_input, punctuated::Punctuated, token::Comma, DeriveInput,
        Expr, Field, Fields, GenericParam, Generics, Type,
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
                 wincode codecs can change the serialized layout; remove the override or compute \
                 the account size manually",
            )
            .to_compile_error()
            .into();
        }
        Ok(None) => {}
        Err(err) => return err.to_compile_error().into(),
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let limit = fresh_ident("__ANCHOR_SPACE_LIMIT", &input.generics);
    let tail = fresh_ident("__AnchorSpaceTail", &input.generics);
    let mut bounded_generics = input.generics.clone();
    bounded_generics
        .params
        .push(syn::parse_quote!(const #limit: usize));
    bounded_generics
        .params
        .push(syn::parse_quote!(#tail: anchor_lang::__private::SpaceLimits));
    let (bounded_impl_generics, _, bounded_where_clause) = bounded_generics.split_for_impl();
    let bounded_name = &input.ident;
    let bounded_impl = if matches!(input.data, syn::Data::Union(_)) {
        quote! {}
    } else {
        quote! {
            #[automatically_derived]
            impl #bounded_impl_generics anchor_lang::__private::BoundedSpace<anchor_lang::__private::Limits<#limit, #tail>>
                for #bounded_name #ty_generics #bounded_where_clause
            {
                const SPACE: usize = <Self as anchor_lang::Space>::INIT_SPACE;
                type Remaining = anchor_lang::__private::Limits<#limit, #tail>;
            }
        }
    };
    let name = input.ident;

    let process_struct_fields = |fields: Punctuated<Field, Comma>| {
        let recurse = fields.into_iter().map(|field| field_len_tokens(field));

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
                let len = v.fields.into_iter().map(|field| field_len_tokens(field));

                quote! {
                    0 #(+ #len)*
                }
            });

            let max = gen_max(variants);

            quote! {
                #[automatically_derived]
                impl #impl_generics anchor_lang::Space for #name #ty_generics #where_clause {
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

    TokenStream::from(quote! { #expanded #bounded_impl })
}

fn fresh_ident(base: &str, generics: &Generics) -> syn::Ident {
    let mut name = base.to_owned();
    while generics.params.iter().any(|param| match param {
        GenericParam::Type(param) => param.ident == name,
        GenericParam::Const(param) => param.ident == name,
        GenericParam::Lifetime(_) => false,
    }) {
        name.push('_');
    }
    format_ident!("{name}")
}

fn field_len_tokens(field: Field) -> TokenStream2 {
    match crate::find_unsupported_wincode_attr(&field.attrs) {
        Ok(Some((crate::UnsupportedWincodeAttrKind::Skip, span))) => {
            return syn::Error::new(
                span,
                "#[derive(InitSpace)] does not support `#[wincode(skip)]` fields because wincode \
                 field overrides change the serialized layout; remove the override or compute the \
                 account size manually",
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

    if !matches!(field.ty, Type::Array(_) | Type::Path(_) | Type::Tuple(_)) {
        return syn::Error::new_spanned(
            &field.ty,
            "#[derive(InitSpace)] can't compute size for this type — use a fixed-size alternative \
             or a bounded owned collection",
        )
        .to_compile_error();
    }
    let attrs: Vec<_> = field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("max_len"))
        .collect();
    if attrs.len() > 1 {
        return syn::Error::new_spanned(attrs[1], "max_len already provided").to_compile_error();
    }
    let capacities = match attrs.first() {
        Some(attr) => match attr.parse_args_with(parse_len_args) {
            Ok(values) => values,
            Err(err) => return err.to_compile_error(),
        },
        None => Vec::new(),
    };
    let mut limits = quote! { anchor_lang::__private::NoLimits };
    for capacity in capacities.iter().rev() {
        limits = quote! { anchor_lang::__private::Limits<{ (#capacity) as usize }, #limits> };
    }
    let ty = field.ty;
    quote! {{
        assert!(<<#ty as anchor_lang::__private::BoundedSpace<#limits>>::Remaining as anchor_lang::__private::SpaceLimits>::IS_EMPTY, "too many max_len capacities");
        <#ty as anchor_lang::__private::BoundedSpace<#limits>>::SPACE
    }}
}

fn parse_len_args(input: ParseStream) -> syn::Result<Vec<Expr>> {
    Ok(Punctuated::<Expr, Comma>::parse_terminated(input)?
        .into_iter()
        .collect())
}

fn gen_max<T: Iterator<Item = TokenStream2>>(mut iter: T) -> TokenStream2 {
    if let Some(item) = iter.next() {
        let next_item = gen_max(iter);
        quote!(anchor_lang::__private::max(#item, #next_item))
    } else {
        quote!(0)
    }
}
