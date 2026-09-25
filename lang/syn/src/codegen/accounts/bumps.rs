use {
    super::constraints,
    crate::{
        codegen::accounts::{generics, ParsedGenerics},
        *,
    },
};

pub fn generate_bumps_name(path: &syn::Path) -> proc_macro2::TokenStream {
    let mut bumps_path = path.clone();
    for segment in &mut bumps_path.segments {
        segment.arguments = syn::PathArguments::None;
    }

    // Composite account fields are guaranteed to be non-empty type paths by the parser.
    let Some(last_segment) = bumps_path.segments.last_mut() else {
        unreachable!("composite account type path must not be empty")
    };
    let location = last_segment.ident.span();
    last_segment.ident = Ident::new(
        &format!("{}Bumps", last_segment.ident),
        Span::call_site().located_at(location),
    );

    quote! { #bumps_path }
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
                    let syn::Type::Path(ty_path) = &s.raw_field.ty else {
                        unreachable!("composite account type must be a path")
                    };
                    let comp_bumps_struct = generate_bumps_name(&ty_path.path);
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
