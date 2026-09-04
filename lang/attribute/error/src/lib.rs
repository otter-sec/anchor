extern crate proc_macro;

use {
    anchor_syn::{
        codegen,
        parser::error::{self as error_parser, ErrorInput},
        ErrorArgs,
    },
    proc_macro::TokenStream,
    quote::quote,
    syn::parse_macro_input,
};

/// Generates `Error` and `type Result<T> = Result<T, Error>` types to be
/// used as return types from Anchor instruction handlers. Importantly, the
/// attribute implements
/// [`From`](https://doc.rust-lang.org/std/convert/trait.From.html) on the
/// `ErrorCode` to support converting from the user defined error enum *into*
/// the generated `Error`.
///
/// # Example
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[program]
/// mod errors {
///     use super::*;
///     pub fn hello(_ctx: Context<Hello>) -> Result<()> {
///         Err(error!(MyError::Hello))
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct Hello {}
///
/// #[error_code]
/// pub enum MyError {
///     #[msg("This is an error message clients will automatically display")]
///     Hello,
/// }
/// ```
///
/// Note that we generate a new `Error` type so that we can return either the
/// user defined error enum *or* a
/// [`ProgramError`](../solana_program/enum.ProgramError.html), which is used
/// pervasively, throughout solana program crates. The generated `Error` type
/// should almost never be used directly, as the user defined error is
/// preferred. In the example above, `error!(MyError::Hello)`.
///
/// # Msg
///
/// The `#[msg(..)]` attribute is inert, and is used only as a marker so that
/// parsers  and IDLs can map error codes to error messages.
#[proc_macro_attribute]
pub fn error_code(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args = match args.is_empty() {
        true => None,
        false => Some(parse_macro_input!(args as ErrorArgs)),
    };
    let mut error_enum = parse_macro_input!(input as syn::ItemEnum);
    let error = match error_parser::parse(&mut error_enum, args) {
        Ok(e) => codegen::error::generate(e),
        Err(e) => e.into_compile_error(),
    };
    proc_macro::TokenStream::from(error)
}

/// Generates an [`Error::AnchorError`](../../anchor_lang/error/enum.Error.html) that includes file and line information.
///
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[program]
/// mod errors {
///     use super::*;
///     pub fn example(_ctx: Context<Example>) -> Result<()> {
///         Err(error!(MyError::Hello))
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct Example {}
///
/// #[error_code]
/// pub enum MyError {
///     #[msg("This is an error message clients will automatically display")]
///     Hello,
/// }
/// ```
#[proc_macro]
pub fn error(ts: proc_macro::TokenStream) -> TokenStream {
    let input = parse_macro_input!(ts as ErrorInput);
    let error_code = input.error_code;
    TokenStream::from(quote! {
        anchor_lang::__anchor_error!(#error_code, source)
    })
}
