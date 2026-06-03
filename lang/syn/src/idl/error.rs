use {
    super::common::{gen_print_section, get_idl_module_path},
    crate::Error,
    heck::SnakeCase,
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
};

pub fn gen_idl_print_fn_error(error: &Error) -> TokenStream {
    let idl = get_idl_module_path();

    let fn_name = format_ident!(
        "__anchor_private_print_idl_error_{}",
        error.ident.to_string().to_snake_case()
    );
    let offset = match &error.args {
        Some(args) => {
            let offset = &args.offset;
            quote! { #offset }
        }
        None => quote! { ::anchor_lang::error::ERROR_CODE_OFFSET },
    };

    let error_codes = error
        .codes
        .iter()
        .map(|code| {
            let id = code.id;
            let name = code.ident.to_string();
            let msg = match &code.msg {
                Some(msg) => quote! { Some(#msg.into()) },
                None => quote! { None },
            };

            quote! {
                #idl::IdlErrorCode {
                    code: #offset + #id,
                    name: #name.into(),
                    msg: #msg,
                }
            }
        })
        .collect::<Vec<_>>();
    let fn_body = gen_print_section("errors", quote! { vec![#(#error_codes),*] });

    quote! {
        #[test]
        pub fn #fn_name() {
            #fn_body
        }
    }
}
