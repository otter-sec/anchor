use {super::common::gen_print_section, proc_macro2::TokenStream, quote::quote};

pub fn gen_idl_print_fn_address() -> TokenStream {
    // Print the value of `ID`, not the source text of whatever was passed to
    // `declare_id!`. The generated function is a `#[test]`, so it runs on the
    // host with `ID` already in scope and fully evaluated — which makes this
    // correct for non-literal ids (`env!`, consts, any expression) where
    // stringifying the macro input produced a mangled address.
    let fn_body = gen_print_section("address", quote! { ID.to_string() });

    quote! {
        #[test]
        pub fn __anchor_private_print_idl_address() {
            #fn_body
        }
    }
}
