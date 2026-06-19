use {
    crate::AnyResult as Result,
    proc_macro2::TokenStream,
    quote::{quote, ToTokens},
    std::path::{Path, PathBuf},
};

pub fn find_path(name: &str, path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    for ancestor in path.ancestors() {
        let file_path = ancestor.join(name);
        if file_path.exists() {
            return file_path.canonicalize().map_err(Into::into);
        }
    }

    Err(format!("Path ({path:?}) not found").into())
}

pub fn get_no_docs() -> bool {
    option_env!("ANCHOR_IDL_BUILD_NO_DOCS")
        .map(|val| val == "TRUE")
        .unwrap_or_default()
}

pub fn get_program_path() -> Result<PathBuf> {
    std::env::var("ANCHOR_IDL_BUILD_PROGRAM_PATH")
        .map(PathBuf::from)
        .map_err(|e| format!("Failed to get program path: {e}").into())
}

pub fn get_idl_module_path() -> TokenStream {
    quote!(anchor_lang::idl::types)
}

pub fn get_serde_json_module_path() -> TokenStream {
    quote!(anchor_lang::idl::serde_json)
}

pub fn gen_print_section(name: &str, value: impl ToTokens) -> TokenStream {
    let serde_json = get_serde_json_module_path();
    quote! {
        println!("--- IDL begin {} ---", #name);
        println!("{}", #serde_json::to_string_pretty(&{ #value }).unwrap());
        println!("--- IDL end {} ---", #name);
    }
}
