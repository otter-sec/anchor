extern crate proc_macro;

#[cfg(feature = "event-cpi")]
use anchor_syn::parser::accounts::event_cpi::{add_event_cpi_accounts, EventAuthority};
use {
    anchor_syn::{codegen::program::common::gen_discriminator, Overrides},
    quote::{quote, ToTokens},
    syn::parse_macro_input,
};

/// The event attribute allows a struct to be used with
/// [emit!](./macro.emit.html) so that programs can log significant events in
/// their programs that clients can subscribe to. Currently, this macro is for
/// structs only.
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
/// See the [`emit!` macro](emit!) for an example.
#[proc_macro_attribute]
pub fn event(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args = parse_macro_input!(args as Overrides);
    let event_strct = parse_macro_input!(input as syn::ItemStruct);
    let event_name = &event_strct.ident;

    let discriminator = args
        .discriminator
        .map(|d| d.to_token_stream())
        .unwrap_or_else(|| gen_discriminator("event", event_name));

    let ret = quote! {
        #[derive(AnchorSerialize, AnchorDeserialize)]
        #event_strct

        impl anchor_lang::Event for #event_name {
            fn data(&self) -> Vec<u8> {
                let mut data = Vec::with_capacity(256);
                data.extend_from_slice(#event_name::DISCRIMINATOR);
                self.serialize(&mut data).unwrap();
                data
            }
        }

        impl anchor_lang::Discriminator for #event_name {
            const DISCRIMINATOR: &'static [u8] = #discriminator;
        }
    };

    #[cfg(feature = "idl-build")]
    {
        let idl_build = anchor_syn::idl::gen_idl_print_fn_event(&event_strct);
        return proc_macro::TokenStream::from(quote! {
            #ret
            #idl_build
        });
    }

    #[allow(unreachable_code)]
    proc_macro::TokenStream::from(ret)
}

/// Logs an event that can be subscribed to by clients.
/// Uses the [`sol_log_data`](https://docs.rs/solana-program/latest/solana_program/log/fn.sol_log_data.html)
/// syscall which results in the following log:
/// ```ignore
/// Program data: <Base64EncodedEvent>
/// ```
/// # Example
///
/// ```rust,ignore
/// use anchor_lang::prelude::*;
///
/// // handler function inside #[program]
/// pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
///     emit!(MyEvent {
///         data: 5,
///         label: [1,2,3,4,5],
///     });
///     Ok(())
/// }
///
/// #[event]
/// pub struct MyEvent {
///     pub data: u64,
///     pub label: [u8; 5],
/// }
/// ```
#[proc_macro]
pub fn emit(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let data: proc_macro2::TokenStream = input.into();
    proc_macro::TokenStream::from(quote! {
        {
            anchor_lang::solana_program::log::sol_log_data(&[&anchor_lang::Event::data(&#data)]);
        }
    })
}

/// Log an event by making a self-CPI that can be subscribed to by clients.
///
/// This way of logging events is more reliable than [`emit!`](emit!) because RPCs are less likely
/// to truncate CPI information than program logs.
///
/// Uses a [`invoke_signed`](https://docs.rs/solana-program/latest/solana_program/program/fn.invoke_signed.html)
/// syscall to store the event data in the ledger, which results in the data being stored in the
/// transaction metadata.
///
/// This method requires the usage of an additional PDA to guarantee that the self-CPI is truly
/// being invoked by the same program. Requiring this PDA to be a signer during `invoke_signed`
/// syscall ensures that the program is the one doing the logging.
///
/// The necessary accounts are added to the accounts struct via [`#[event_cpi]`](event_cpi)
/// attribute macro.
///
/// # Example
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[program]
/// pub mod my_program {
///     use super::*;
///
///     pub fn my_instruction(ctx: Context<MyInstruction>) -> Result<()> {
///         emit_cpi!(MyEvent { data: 42 });
///         Ok(())
///     }
/// }
///
/// #[event_cpi]
/// #[derive(Accounts)]
/// pub struct MyInstruction {}
///
/// #[event]
/// pub struct MyEvent {
///     pub data: u64,
/// }
/// ```
///
/// **NOTE:** This macro requires `ctx` to be in scope.
///
/// *Only available with `event-cpi` feature enabled.*
#[cfg(feature = "event-cpi")]
#[proc_macro]
pub fn emit_cpi(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let event_struct = parse_macro_input!(input as syn::Expr);

    let authority = EventAuthority::get();
    let authority_name = authority.name_token_stream();
    let authority_seeds = authority.seeds;

    proc_macro::TokenStream::from(quote! {
        {
            let authority_info = ctx.accounts.#authority_name.to_account_info();

            let disc = anchor_lang::event::EVENT_IX_TAG_LE;
            let inner_data = anchor_lang::Event::data(&#event_struct);
            let ix_data: Vec<u8> = disc
                .into_iter()
                .map(|b| *b)
                .chain(inner_data.into_iter())
                .collect();

            let ix = anchor_lang::solana_program::instruction::Instruction::new_with_bytes(
                // In a doctest the ID will be in the current scope, not the crate root
                #[cfg(not(doctest))]
                { crate::ID },
                #[cfg(doctest)]
                { ID },
                &ix_data,
                vec![
                    anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                        *authority_info.key,
                        true,
                    ),
                ],
            );
            anchor_lang::solana_program::program::invoke_signed(
                &ix,
                &[authority_info],
                &[&[#authority_seeds, &[crate::EVENT_AUTHORITY_AND_BUMP.1]]],
            )
            .map_err(anchor_lang::error::Error::from)?;
        }
    })
}

/// An attribute macro to add necessary event CPI accounts to the given accounts struct.
///
/// Two accounts named `event_authority` and `program` will be appended to the list of accounts.
///
/// # Example
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[event_cpi]
/// #[derive(Accounts)]
/// pub struct MyInstruction<'info> {
///    pub signer: Signer<'info>,
/// }
/// ```
///
/// The code above will be expanded to:
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// #[derive(Accounts)]
/// pub struct MyInstruction<'info> {
///    pub signer: Signer<'info>,
///    /// CHECK: Only the event authority can invoke self-CPI
///    #[account(address = crate::EVENT_AUTHORITY_AND_BUMP.0)]
///    pub event_authority: UncheckedAccount<'info>,
///    /// CHECK: Self-CPI will fail if the program is not the current program
///    pub program: UncheckedAccount<'info>,
/// }
/// ```
///
/// See [`emit_cpi!`](emit_cpi!) for a full example.
///
/// *Only available with `event-cpi` feature enabled.*
#[cfg(feature = "event-cpi")]
#[proc_macro_attribute]
pub fn event_cpi(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let accounts_struct = parse_macro_input!(input as syn::ItemStruct);
    #[allow(
        clippy::unwrap_used,
        reason = "quote-generated struct tokens always parse"
    )]
    let accounts_struct = add_event_cpi_accounts(&accounts_struct).unwrap();
    proc_macro::TokenStream::from(quote! {#accounts_struct})
}
