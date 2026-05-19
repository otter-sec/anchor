use {
    super::{
        common::{gen_print_section, get_idl_module_path, get_no_docs, get_program_path},
        defined::gen_idl_type,
    },
    crate::{
        parser::{context::CrateContext, docs},
        Program,
    },
    anyhow::{anyhow, Result},
    heck::CamelCase,
    proc_macro2::TokenStream,
    quote::{format_ident, quote},
    syn::spanned::Spanned,
};

/// Generate the IDL build print function for the program module.
pub fn gen_idl_print_fn_program(program: &Program) -> TokenStream {
    check_safety_comments().unwrap_or_else(|e| panic!("Safety checks failed: {e}"));

    let idl = get_idl_module_path();
    let no_docs = get_no_docs();

    let name = program.name.to_string();
    let docs = match &program.docs {
        Some(docs) if !no_docs => quote! { vec![#(#docs.into()),*] },
        _ => quote! { vec![] },
    };

    let result = program
        .ixs
        .iter()
        .map(|ix| {
            let name = ix.ident.to_string();
            let name_pascal = format_ident!("{}", name.to_camel_case());
            let ctx_ident = &ix.anchor_ident;
            let cfgs = &ix.cfgs;

            let docs = match &ix.docs {
                Some(docs) if !no_docs => quote! { vec![#(#docs.into()),*] },
                _ => quote! { vec![] },
            };

            let (args, mut defined) = ix
                .args
                .iter()
                .map(|arg| {
                    let name = arg.name.to_string();
                    let docs = match docs::parse(&arg.raw_arg.attrs) {
                        Some(docs) if !no_docs => quote! { vec![#(#docs.into()),*] },
                        _ => quote! { vec![] },
                    };
                    let (ty, defined) = gen_idl_type(&arg.raw_arg.ty, &[])
                        .map_err(|_| syn::Error::new(arg.raw_arg.ty.span(), "Unsupported type"))?;

                    Ok((
                        quote! {
                            #idl::IdlField {
                                name: #name.into(),
                                docs: #docs,
                                ty: #ty,
                            }
                        },
                        defined,
                    ))
                })
                .collect::<syn::Result<Vec<_>>>()?
                .into_iter()
                .unzip::<_, Vec<_>, Vec<_>, Vec<_>>();

            let returns = match gen_idl_type(&ix.returns.ty, &[]) {
                Ok((ty, def)) => {
                    defined.push(def);
                    quote! { Some(#ty) }
                }
                _ => quote! { None },
            };

            Ok((
                quote! {
                    #(#cfgs)*
                    #idl::IdlInstruction {
                        name: #name.into(),
                        docs: #docs,
                        discriminator: crate::instruction::#name_pascal::DISCRIMINATOR.into(),
                        accounts: #ctx_ident::__anchor_private_gen_idl_accounts(
                            &mut accounts,
                            &mut types,
                        ),
                        args: vec![#(#args),*],
                        returns: #returns,
                    }
                },
                defined,
            ))
        })
        .collect::<syn::Result<Vec<_>>>();
    let (instructions, defined) = match result {
        Err(e) => return e.into_compile_error(),
        Ok(v) => v.into_iter().unzip::<_, Vec<_>, Vec<_>, Vec<_>>(),
    };
    let defined = defined.into_iter().flatten().flatten().collect::<Vec<_>>();

    let fn_body = gen_print_section(
        "program",
        quote! {
            let mut accounts: std::collections::BTreeMap<String, #idl::IdlAccount> =
                std::collections::BTreeMap::new();
            let mut types: std::collections::BTreeMap<String, #idl::IdlTypeDef> =
                std::collections::BTreeMap::new();

            #(
                if let Some(ty) = <#defined>::create_type() {
                    types.insert(<#defined>::get_full_path(), ty);
                    <#defined>::insert_types(&mut types);
                }
            );*

            #idl::Idl {
                address: Default::default(),
                metadata: #idl::IdlMetadata {
                    name: #name.into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    spec: #idl::IDL_SPEC.into(),
                    description: option_env!("CARGO_PKG_DESCRIPTION")
                        .filter(|d| !d.is_empty())
                        .map(|d| d.into()),
                    repository: option_env!("CARGO_PKG_REPOSITORY")
                        .filter(|r| !r.is_empty())
                        .map(|r| r.into()),
                    dependencies: Default::default(),
                    contact: Default::default(),
                    deployments: Default::default(),
                },
                docs: #docs,
                instructions: vec![#(#instructions),*],
                accounts: accounts.into_values().collect(),
                events: Default::default(),
                errors: Default::default(),
                types: types.into_values().collect(),
                constants: Default::default(),
            }
        },
    );

    quote! {
        #[test]
        pub fn __anchor_private_print_idl_program() {
            #fn_body
        }
    }
}

/// Check safety comments.
fn check_safety_comments() -> Result<()> {
    let skip_lint = option_env!("ANCHOR_IDL_BUILD_SKIP_LINT")
        .map(|val| val == "TRUE")
        .unwrap_or_default();
    if skip_lint {
        return Ok(());
    }

    let program_path = get_program_path();
    if program_path.is_err() {
        // Getting the program path can fail in the following scenarios:
        //
        // - Anchor CLI version is incompatible with the current version
        // - The error is coming from Rust Analyzer when the user has `idl-build` feature enabled,
        // likely due to enabling all features (https://github.com/otter-sec/anchor/issues/3042)
        //
        // For the first case, we have a warning when the user is using different versions of the
        // lang and CLI crate. For the second case, users would either have to disable the
        // `idl-build` feature, or define the program path environment variable in Rust Analyzer
        // settings.
        //
        // Given this feature is not a critical one, and it works by default with `anchor build`,
        // we can fail silently in the case of an error rather than panicking.
        return Ok(());
    }

    program_path
        .map(|path| path.join("src").join("lib.rs"))
        .map(CrateContext::parse)?
        .map_err(|e| anyhow!("Failed to parse crate: {e}"))?
        .safety_checks()
}
