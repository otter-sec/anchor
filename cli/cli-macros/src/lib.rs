use {
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::{parse_macro_input, DataEnum, DataStruct, DeriveInput, Fields},
};

#[proc_macro_derive(AbsolutePath)]
pub fn absolute_path_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let constructor = match input.data {
        syn::Data::Struct(data_struct) => derive_struct(data_struct),
        syn::Data::Enum(data_enum) => derive_enum(data_enum),
        syn::Data::Union(_) => {
            quote! { compile_error!("unions are not supported") }
        }
    };
    let name = input.ident;

    quote! {
        // Some fields/commands in the CLI are deprecated
        #[allow(deprecated)]
        impl AbsolutePath for #name {
            fn absolute(self) -> Self {
                #constructor
            }
        }
    }
    .into()
}

fn derive_struct(input: DataStruct) -> TokenStream {
    match input.fields {
        Fields::Named(fields) => {
            let members: Vec<_> = fields
                .named
                .into_iter()
                .map(|field| field.ident.expect("named field"))
                .collect();
            quote! {
                Self {
                    #(#members: self.#members.absolute()),*
                }
            }
        }
        Fields::Unnamed(fields) => {
            let members: Vec<_> = (0..fields.unnamed.len()).map(syn::Index::from).collect();
            quote! {
                Self(
                    #(self.#members.absolute()),*
                )
            }
        }
        Fields::Unit => quote! { Self },
    }
}

fn derive_enum(input: DataEnum) -> TokenStream {
    let variants = input.variants.into_iter().map(|variant| {
        let name = variant.ident;
        match variant.fields {
            Fields::Named(fields) => {
                let members: Vec<_> = fields
                    .named
                    .into_iter()
                    .map(|field| field.ident.expect("named field"))
                    .collect();
                quote! {
                    Self::#name { #(#members),* } => Self::#name {
                        #(#members: #members.absolute()),*
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let members: Vec<_> = (0..fields.unnamed.len())
                    .map(|index| format_ident!("field_{index}"))
                    .collect();
                quote! {
                    Self::#name(#(#members),*) => Self::#name(
                        #(#members.absolute()),*
                    )
                }
            }
            Fields::Unit => quote! {
                Self::#name => Self::#name
            },
        }
    });
    quote! {
        match self {
            #(#variants),*
        }
    }
}
