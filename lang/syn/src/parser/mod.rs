pub mod accounts;
pub mod context;
pub mod docs;
pub mod error;
pub mod program;

pub fn tts_to_string<T: quote::ToTokens>(item: T) -> String {
    item.to_token_stream().to_string()
}

/// Matches an exact `bytemuck::<expected_leaf>` derive path, nothing shorter or longer.
pub fn is_bytemuck_derive(path: &syn::Path, expected_leaf: &str) -> bool {
    let mut segments = path.segments.iter();

    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(first), Some(second), None)
            if first.ident == "bytemuck" && second.ident == expected_leaf
    )
}
