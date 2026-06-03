use {
    proc_macro2::Literal,
    quote::{format_ident, quote, ToTokens},
    syn::{parse_quote, spanned::Spanned, Fields, Item},
};

pub fn gen_lazy(input: proc_macro::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let item = syn::parse::<Item>(input)?;
    let (name, generics, size, sized) = match &item {
        Item::Struct(strct) => (
            &strct.ident,
            &strct.generics,
            sum_fields(&strct.fields),
            strct
                .fields
                .iter()
                .map(|field| &field.ty)
                .map(|ty| quote! { <#ty as anchor_lang::__private::Lazy>::SIZED })
                .fold(quote!(true), |acc, sized| quote! { #acc && #sized }),
        ),
        Item::Enum(enm) => {
            // Each match arm checks the tag byte then shadows the `buf` variable with
            // the remaining buffer. This allows `sum_fields` to operate on the correct buffer
            let arms = enm
                .variants
                .iter()
                .map(|variant| sum_fields(&variant.fields))
                .enumerate()
                .map(|(i, size)| (Literal::usize_unsuffixed(i), size))
                .map(|(i, size)| quote! { Some((#i, buf)) => { #size } });
            let sized = enm
                .variants
                .iter()
                .all(|variant| matches!(variant.fields, Fields::Unit));
            (
                &enm.ident,
                &enm.generics,
                quote! {
                    1 + match buf.split_first() {
                        #(#arms,)*
                        _ => unreachable!(),
                    }
                },
                sized.to_token_stream(),
            )
        }
        Item::Union(_) => return Err(syn::Error::new(item.span(), "Unions are not supported")),
        _ => unreachable!(),
    };

    let mut generics = generics.clone();
    for ty in generics.type_params_mut() {
        ty.bounds.push(parse_quote!(anchor_lang::__private::Lazy));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote! {
        impl #impl_generics anchor_lang::__private::Lazy for #name #ty_generics #where_clause {
            const SIZED: bool = #sized;

            #[inline(always)]
            fn size_of(buf: &[u8]) -> usize {
                #size
            }
        }
    })
}

fn sum_fields(fields: &Fields) -> proc_macro2::TokenStream {
    let names = fields
        .iter()
        .enumerate()
        .map(|(i, _)| format_ident!("s{i}"))
        .collect::<Vec<_>>();
    let declarations = fields.iter().enumerate().map(|(i, field)| {
        let ty = &field.ty;
        let name = &names[i];
        let sum = &names[..i];
        let buf = quote! { &buf[0 #(+ #sum)*..] };
        quote! { let #name = <#ty as anchor_lang::__private::Lazy>::size_of(#buf) }
    });

    quote! {
       #(#declarations;)*
       0 #(+ #names)*
    }
}
