use {
    super::{
        common::{gen_print_section, get_idl_module_path, get_serde_json_module_path},
        defined::gen_idl_type_def_struct,
    },
    heck::SnakeCase,
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

pub fn gen_idl_print_fn_event(event_struct: &syn::ItemStruct) -> TokenStream {
    let idl = get_idl_module_path();
    let serde_json = get_serde_json_module_path();

    let ident = &event_struct.ident;
    let fn_name = format_ident!(
        "__anchor_private_print_idl_event_{}",
        ident.to_string().to_snake_case()
    );
    let idl_build_impl = impl_idl_build_event(event_struct);

    let print_ts = gen_print_section(
        "event",
        quote! {
            #serde_json::json!({
                "event": event,
                "types": types.into_values().collect::<Vec<_>>()
            })
        },
    );

    quote! {
        #idl_build_impl

        #[test]
        pub fn #fn_name() {
            let mut types: ::std::collections::BTreeMap<String, #idl::IdlTypeDef> =
                ::std::collections::BTreeMap::new();
            if let Some(event) = #ident::__anchor_private_gen_idl_event(&mut types) {
                #print_ts
            }
        }
    }
}

/// Generate IDL build impl for an event.
fn impl_idl_build_event(event_struct: &syn::ItemStruct) -> TokenStream {
    let idl = get_idl_module_path();

    let ident = &event_struct.ident;
    let (impl_generics, ty_generics, where_clause) = event_struct.generics.split_for_impl();

    let fn_body = match gen_idl_type_def_struct(event_struct) {
        Ok((ts, defined)) => quote! {
            #(
                if let Some(ty) = <#defined>::create_type() {
                    types.insert(<#defined>::get_full_path(), ty);
                    <#defined>::insert_types(types);
                }
            );*

            let ty = #ts;
            let event = #idl::IdlEvent {
                name: ty.name.clone(),
                discriminator: Self::DISCRIMINATOR.into(),
            };
            types.insert(ty.name.clone(), ty);
            Some(event)
        },
        _ => quote! { None },
    };

    quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn __anchor_private_gen_idl_event(
                types: &mut ::std::collections::BTreeMap<String, #idl::IdlTypeDef>,
            ) -> Option<#idl::IdlEvent> {
                #fn_body
            }
        }
    }
}
