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
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "None" && segment.arguments.is_empty())
}
