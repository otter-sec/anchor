extern crate proc_macro;

use {quote::quote, syn::parse_macro_input};

/// Executes the given access control method before running the decorated
/// instruction handler. Any method in scope of the attribute can be invoked
/// with any arguments from the associated instruction handler.
///
/// # Example
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[program]
/// mod errors {
///     use super::*;
///
///     #[access_control(Create::accounts(&ctx, bump_seed))]
///     pub fn create(ctx: Context<Create>, bump_seed: u8) -> Result<()> {
///       let my_account = &mut ctx.accounts.my_account;
///       my_account.bump_seed = bump_seed;
///       Ok(())
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct Create<'info> {
///   #[account(init, payer = payer, space = 8 + 1)]
///   my_account: Account<'info, MyAccount>,
///   #[account(mut)]
///   payer: Signer<'info>,
///   system_program: Program<'info, System>,
/// }
///
/// #[account]
/// pub struct MyAccount {
///     bump_seed: u8,
/// }
///
/// impl Create<'_> {
///   pub fn accounts(ctx: &Context<Create>, bump_seed: u8) -> Result<()> {
///     let seeds = &[ctx.accounts.my_account.to_account_info().key.as_ref(), &[bump_seed]];
///     Pubkey::create_program_address(seeds, ctx.program_id)
///       .map_err(|_| error!(ErrorCode::InvalidNonce))?;
///     Ok(())
///   }
/// }
/// ```
///
/// This example demonstrates a useful pattern. Not only can you use
/// `#[access_control]` to ensure any invariants or preconditions hold prior to
/// executing an instruction, but also it can be used to finish any validation
/// on the `Accounts` struct, particularly when instruction arguments are
/// needed. Here, we use the given `bump_seed` to verify it creates a valid
/// program-derived address.
#[proc_macro_attribute]
pub fn access_control(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut args = args.to_string();
    args.retain(|c| !c.is_whitespace());
    let access_control: Vec<proc_macro2::TokenStream> = args
        .split(')')
        .filter(|ac| !ac.is_empty())
        .map(|ac| format!("{ac})")) // Put back on the split char.
        .map(|ac| format!("{ac}?;")) // Add `?;` syntax.
        .map(|ac| {
            ac.parse::<proc_macro2::TokenStream>().map_err(|_| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("`#[access_control]` argument `{ac} is not valid Rust syntax"),
                )
                .into_compile_error()
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| vec![err]);

    let item_fn = parse_macro_input!(input as syn::ItemFn);

    let fn_attrs = item_fn.attrs;
    let fn_vis = item_fn.vis;
    let fn_sig = item_fn.sig;
    let fn_block = item_fn.block;

    let fn_stmts = fn_block.stmts;

    proc_macro::TokenStream::from(quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig {

            #(#access_control)*

            #(#fn_stmts)*
        }
    })
}
