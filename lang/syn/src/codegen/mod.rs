pub mod accounts;
pub mod error;
pub mod program;

/// Creates an identifier for implementation details emitted by a procedural macro.
///
/// Mixed-site hygiene keeps generated bindings distinct from user identifiers with
/// the same spelling while still allowing generated references to resolve them.
pub(crate) fn private_ident(name: &str) -> syn::Ident {
    syn::Ident::new(name, proc_macro2::Span::mixed_site())
}
