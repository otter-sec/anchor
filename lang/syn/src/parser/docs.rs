// returns vec of doc strings
pub fn parse(attrs: &[syn::Attribute]) -> Option<Vec<String>> {
    let doc_strings: Vec<String> = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            if let syn::Meta::NameValue(syn::MetaNameValue {
                value:
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(doc),
                        ..
                    }),
                ..
            }) = &attr.meta
            {
                let val = doc.value().trim().to_string();
                if val.starts_with("CHECK:") {
                    return None;
                }
                return Some(val);
            }
            None
        })
        .collect();
    if doc_strings.is_empty() {
        None
    } else {
        Some(doc_strings)
    }
}
