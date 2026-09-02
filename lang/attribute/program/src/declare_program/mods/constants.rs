use {
    super::common::{convert_idl_type_to_str, gen_docs},
    anchor_lang_idl::types::{Idl, IdlType},
    quote::{format_ident, quote, ToTokens},
    syn::visit_mut::VisitMut,
};

pub fn gen_constants_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let defined_paths = get_defined_paths(idl);
    let constants = idl.constants.iter().map(|c| {
        let name = format_ident!("{}", c.name);
        let docs = gen_docs(&c.docs);
        #[allow(
            clippy::unwrap_used,
            reason = "compile_error! token stream is always valid syn::Type syntax"
        )]
        let ty = convert_idl_type_to_str(&c.ty, true)
            .and_then(|s| {
                syn::parse_str::<syn::Type>(&s)
                    .map_err(|err| syn::Error::new(proc_macro2::Span::call_site(), err.to_string()))
            })
            .unwrap_or_else(|err| syn::parse2(err.into_compile_error()).unwrap());
        #[allow(
            clippy::unwrap_used,
            reason = "IDL constant values are valid Rust expressions by construction"
        )]
        let mut val = syn::parse_str::<syn::Expr>(&c.value).unwrap();
        DefinedPathQualifier::new(&defined_paths).visit_expr_mut(&mut val);
        let val = val.to_token_stream();
        let val = match &c.ty {
            IdlType::Bytes => quote! { &#val },
            IdlType::Pubkey => quote!(anchor_lang::prelude::Pubkey::from_str_const(
                stringify!(#val)
            )),
            _ => val,
        };

        quote! {
            #docs
            pub const #name: #ty = #val;
        }
    });

    quote! {
        /// Program constants.
        pub mod constants {
            use super::*;

            #(#constants)*
        }
    }
}

fn get_defined_paths(idl: &Idl) -> Vec<Vec<String>> {
    let mut paths = idl
        .accounts
        .iter()
        .map(|acc| acc.name.as_str())
        .chain(idl.events.iter().map(|event| event.name.as_str()))
        .chain(idl.types.iter().map(|ty| ty.name.as_str()))
        .map(|name| name.split("::").map(str::to_owned).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    paths.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    paths.dedup();
    paths
}

struct DefinedPathQualifier<'a> {
    defined_paths: &'a [Vec<String>],
}

impl<'a> DefinedPathQualifier<'a> {
    fn new(defined_paths: &'a [Vec<String>]) -> Self {
        Self { defined_paths }
    }

    fn qualify_path(&self, path: &mut syn::Path) {
        if path.leading_colon.is_some() || path.segments.is_empty() {
            return;
        }

        let Some(first_segment) = path.segments.first() else {
            return;
        };
        let first = first_segment.ident.to_string();
        if matches!(first.as_str(), "__defined" | "crate" | "self" | "super") {
            return;
        }

        let path_idents = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        let should_qualify = self
            .defined_paths
            .iter()
            .any(|defined| path_idents.starts_with(defined));
        if !should_qualify {
            return;
        }

        let old_segments = path.segments.clone();
        path.segments = syn::punctuated::Punctuated::new();
        path.segments.push(format_ident!("__defined").into());
        path.segments.extend(old_segments);
    }
}

impl VisitMut for DefinedPathQualifier<'_> {
    fn visit_expr_path_mut(&mut self, expr_path: &mut syn::ExprPath) {
        syn::visit_mut::visit_expr_path_mut(self, expr_path);
        if expr_path.qself.is_none() {
            self.qualify_path(&mut expr_path.path);
        }
    }

    fn visit_expr_struct_mut(&mut self, expr_struct: &mut syn::ExprStruct) {
        syn::visit_mut::visit_expr_struct_mut(self, expr_struct);
        self.qualify_path(&mut expr_struct.path);
    }
}
