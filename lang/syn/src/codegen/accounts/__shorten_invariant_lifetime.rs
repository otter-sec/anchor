use {
    super::{generics, ParsedGenerics},
    crate::AccountsStruct,
    quote::quote,
};

pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    let ParsedGenerics {
        combined_generics,
        trait_generics: _,
        struct_generics,
        where_clause,
    } = generics(accs);

    let shorten_invariant_lifetime = if accs.generics.lt_token.is_some() {
        let non_lifetime_generics = struct_generics
            .iter()
            .skip_while(|g| matches!(g, syn::GenericParam::Lifetime(_)))
            .fold(quote! {}, |acc, g| quote! { #acc, #g });
        quote! {
            pub unsafe fn __shorten_invariant_lifetime<'__a, '__info: '__a>(
                value: &'__a mut #name<'__info #non_lifetime_generics>,
            ) -> &'__a mut #name<'__a #non_lifetime_generics> {
                unsafe { ::core::mem::transmute(value) }
            }
        }
    } else {
        quote! {
            pub fn __shorten_invariant_lifetime(value: &mut Self) -> &mut Self {
                value
            }
        }
    };

    quote! {
        #[automatically_derived]
        impl<#combined_generics> #name<#struct_generics> #where_clause {
            #[doc(hidden)]
            #[inline(always)]
            #shorten_invariant_lifetime
        }
    }
}
