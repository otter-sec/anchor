use {
    crate::idl::rust_type_to_idl,
    proc_macro::TokenStream,
    quote::{format_ident, quote},
};

pub fn expand(input: TokenStream) -> TokenStream {
    let item: syn::Item = match syn::parse(input.clone()) {
        Ok(item) => item,
        Err(_) => return input,
    };

    let item_const = match &item {
        syn::Item::Const(ic) => ic,
        _ => return quote! { #item }.into(),
    };

    let name = item_const.ident.to_string();
    let ty_json = rust_type_to_idl(&item_const.ty);
    let expr = &item_const.expr;
    let fn_name = format_ident!("__anchor_private_print_idl_const_{}", name.to_lowercase());

    let idl_print = quote! {
        #[cfg(all(test, feature = "idl-build"))]
        #[test]
        fn #fn_name() {
            let value = format!("{:?}", #expr);
            let value_json = anchor_lang::idl_build::__idl_json_string(&value);
            println!("--- IDL begin const ---");
            println!(
                "{{\"name\":\"{}\",\"type\":{},\"value\":{}}}",
                #name, #ty_json, value_json,
            );
            println!("--- IDL end const ---");
        }
    };

    quote! {
        #item_const
        #idl_print
    }
    .into()
}
