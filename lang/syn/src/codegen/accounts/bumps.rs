use {
    super::constraints,
    crate::{
        codegen::accounts::{generics, ParsedGenerics},
        *,
    },
};

pub fn generate_bumps_name(anchor_ident: &str) -> proc_macro2::TokenStream {
    if let Some((prefix, name)) = anchor_ident.rsplit_once("::") {
        #[allow(
            clippy::unwrap_used,
            clippy::expect_used,
            reason = "prefix is derived from a valid Rust path string"
        )]
        let prefix: proc_macro2::TokenStream = prefix
            .parse()
            .expect("module prefix must be a valid Rust path");
        let bumps_name = format!("{name}Bumps");
        let bumps_ident = Ident::new(&bumps_name, Span::call_site());
        quote! { #prefix :: #bumps_ident }
    } else {
        let bumps_name = format!("{anchor_ident}Bumps");
        let bumps_ident = Ident::new(&bumps_name, Span::call_site());
        quote! { #bumps_ident }
    }
}

pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    let bumps_name = Ident::new(&format!("{name}Bumps"), Span::call_site());
    let ParsedGenerics {
        combined_generics,
        trait_generics: _,
        struct_generics,
        where_clause,
    } = generics(accs);

    let (bump_fields, bump_default_fields): (
        Vec<proc_macro2::TokenStream>,
        Vec<proc_macro2::TokenStream>,
    ) = accs
        .fields
        .iter()
        .filter_map(|af| {
            let ident = af.ident();

            match af {
                AccountField::Field(f) => {
                    let constraints = constraints::linearize(&f.constraints);
                    let (bump_field, bump_default_field) = if f.is_optional {
                        (quote!(pub #ident: Option<u8>), quote!(#ident: None))
                    } else {
                        (quote!(pub #ident: u8), quote!(#ident: u8::MAX))
                    };

                    for c in constraints.iter() {
                        // Verify this in super::constraints
                        // The bump is only cached if
                        // - PDA is marked as init
                        // - PDA is not init, but marked with bump without a target

                        match c {
                            Constraint::Seeds(c) if !c.is_init && c.bump.is_none() => {
                                return Some((bump_field, bump_default_field));
                            }
                            Constraint::Init(c) if c.seeds.is_some() => {
                                return Some((bump_field, bump_default_field));
                            }
                            _ => (),
                        }
                    }
                    None
                }
                AccountField::CompositeField(s) => {
                    let comp_bumps_struct = generate_bumps_name(&s.symbol);
                    let bumps = quote!(pub #ident: #comp_bumps_struct);
                    let bumps_default = quote!(#ident: #comp_bumps_struct::default());

                    Some((bumps, bumps_default))
                }
            }
        })
        .unzip();

    quote! {
        #[derive(Debug, Clone, Copy)]
        pub struct #bumps_name {
            #(#bump_fields),*
        }

        impl Default for #bumps_name {
            fn default() -> Self {
                #bumps_name {
                    #(#bump_default_fields),*
                }
            }
        }

        impl<#combined_generics> anchor_lang::Bumps for #name<#struct_generics> #where_clause {
            type Bumps = #bumps_name;
        }
    }
}
