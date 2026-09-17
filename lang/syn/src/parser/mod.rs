pub mod accounts;
pub mod context;
pub mod docs;
pub mod error;
pub mod program;

use syn::Expr;

pub fn tts_to_string<T: quote::ToTokens>(item: T) -> String {
    item.to_token_stream().to_string()
}

/// Returns true for the `None` literal (`None`, `Option::None`, …).
pub fn expr_is_none(expr: &Expr) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let idents = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    matches!(
        idents
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .as_slice(),
        ["None"]
            | ["Option", "None"]
            | ["std", "option", "Option", "None"]
            | ["core", "option", "Option", "None"]
    )
}
