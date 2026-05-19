extern crate proc_macro;

mod declare_program;

use {declare_program::DeclareProgram, quote::ToTokens, syn::parse_macro_input};

/// The `#[program]` attribute defines the module containing all instruction
/// handlers defining all entries into a Solana program.
#[proc_macro_attribute]
pub fn program(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let program = parse_macro_input!(input as anchor_syn::Program);
    let program_tokens = program.to_token_stream();

    #[cfg(feature = "idl-build")]
    {
        use anchor_syn::idl::gen_idl_print_fn_program;
        let idl_tokens = gen_idl_print_fn_program(&program);
        return proc_macro::TokenStream::from(quote::quote! {
            #program_tokens
            #idl_tokens
        });
    }

    #[allow(unreachable_code)]
    proc_macro::TokenStream::from(program_tokens)
}

/// Declare an external program based on its IDL.
///
/// The IDL of the program must exist in a directory named `idls`. This directory can be at any
/// depth, e.g. both inside the program's directory (`<PROGRAM_DIR>/idls`) and inside Anchor
/// workspace root directory (`<PROGRAM_DIR>/../../idls`) are valid.
///
/// # Usage
///
/// ```rs
/// declare_program!(program_name);
/// ```
///
/// This generates a module named `program_name` that can be used to interact with the program
/// without having to add the program's crate as a dependency.
///
/// Both on-chain and off-chain usage is supported.
///
/// Use `cargo doc --open` to see the generated modules and their documentation.
///
/// # Note
///
/// Re-defining the same program to use the same definitions should be avoided since this results
/// in a larger binary size.
///
/// A program should only be defined once. If you have multiple programs that depend on the same
/// definition, you should consider creating a separate crate for the external program definition
/// and reusing it in your programs.
///
/// # Off-chain usage
///
/// When using in off-chain environments, for example via `anchor_client`, you must have
/// `anchor_lang` in scope:
///
/// ```ignore
/// use anchor_client::anchor_lang;
/// ```
///
/// # Example
///
/// A full on-chain CPI usage example can be found [here].
///
/// [here]: https://github.com/otter-sec/anchor/tree/v1.0.2/tests/declare-program
#[proc_macro]
pub fn declare_program(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    parse_macro_input!(input as DeclareProgram)
        .to_token_stream()
        .into()
}

/// This attribute is used to override the Anchor defaults of program instructions.
///
/// # Arguments
///
/// - `discriminator`: Override the default 8-byte discriminator
///
///     **Usage:** `discriminator = <CONST_EXPR>`
///
///     All constant expressions are supported.
///
///     **Examples:**
///
///     - `discriminator = 1` (shortcut for `[1]`)
///     - `discriminator = [1, 2, 3, 4]`
///     - `discriminator = b"hi"`
///     - `discriminator = MY_DISC`
///     - `discriminator = get_disc(...)`
///
/// # Example
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// declare_id!("CustomDiscriminator111111111111111111111111");
///
/// #[program]
/// pub mod custom_discriminator {
///     use super::*;
///
///     #[instruction(discriminator = [1, 2, 3, 4])]
///     pub fn my_ix(_ctx: Context<MyIx>) -> Result<()> {
///         Ok(())
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct MyIx<'info> {
///     pub signer: Signer<'info>,
/// }
/// ```
#[proc_macro_attribute]
pub fn instruction(
    _args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // This macro itself is a no-op, but the `#[program]` macro will detect this attribute and use
    // the arguments to transform the instruction.
    input
}
