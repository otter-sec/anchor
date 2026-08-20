use {
    super::common::{get_all_instruction_accounts, get_canonical_program_id},
    anchor_lang_idl::types::{Idl, IdlInstructionAccountItem, IdlInstructionAccounts},
    heck::CamelCase,
    quote::{format_ident, quote},
};

pub fn gen_parsers_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let account = gen_account(idl);
    let event = gen_event(idl);
    let instruction = gen_instruction(idl);

    quote! {
        /// Program parsers.
        #[cfg(not(target_os = "solana"))]
        pub mod parsers {
            use super::*;

            #account
            #event
            #instruction
        }
    }
}

fn gen_account(idl: &Idl) -> proc_macro2::TokenStream {
    let variants = idl
        .accounts
        .iter()
        .map(|acc| format_ident!("{}", acc.name))
        .map(|name| quote! { #name(super::accounts::#name) });
    let if_statements = idl.accounts.iter().map(|acc| {
        let name = format_ident!("{}", acc.name);
        quote! {
            if value.starts_with(super::accounts::#name::DISCRIMINATOR) {
                return super::accounts::#name::try_deserialize_unchecked(&mut &value[..])
                    .map(Self::#name)
                    .map_err(Into::into)
            }
        }
    });

    quote! {
        /// An enum that includes all accounts of the declared program as a tuple variant.
        ///
        /// See [`Self::parse`] to create an instance from account data.
        pub enum Account {
            #(#variants,)*
        }

        impl Account {
            /// Parse an account based on the given account data.
            ///
            /// This method returns an error if the discriminator of the given bytes don't match
            /// with any of the existing accounts, or if the deserialization fails.
            pub fn parse(data: &[u8]) -> Result<Self> {
                Self::try_from(data)
            }
        }

        impl TryFrom<&[u8]> for Account {
            type Error = anchor_lang::error::Error;

            fn try_from(value: &[u8]) -> Result<Self> {
                #(#if_statements)*
                Err(ProgramError::InvalidArgument.into())
            }
        }
    }
}

fn gen_event(idl: &Idl) -> proc_macro2::TokenStream {
    let variants = idl
        .events
        .iter()
        .map(|ev| format_ident!("{}", ev.name))
        .map(|name| quote! { #name(super::events::#name) });
    let if_statements = idl.events.iter().map(|ev| {
        let name = format_ident!("{}", ev.name);
        quote! {
            if value.starts_with(super::events::#name::DISCRIMINATOR) {
                return super::events::#name::try_from_slice(&value[super::events::#name::DISCRIMINATOR.len()..])
                    .map(Self::#name)
                    .map_err(Into::into)
            }
        }
    });

    quote! {
        /// An enum that includes all events of the declared program as a tuple variant.
        ///
        /// See [`Self::parse`] to create an instance from event data.
        pub enum Event {
            #(#variants,)*
        }

        impl Event {
            /// Parse an event based on the given event data.
            ///
            /// This method returns an error if the discriminator of the given bytes don't match
            /// with any of the existing events, or if the deserialization fails.
            pub fn parse(data: &[u8]) -> Result<Self> {
                Self::try_from(data)
            }
        }

        impl TryFrom<&[u8]> for Event {
            type Error = anchor_lang::error::Error;

            fn try_from(value: &[u8]) -> Result<Self> {
                #(#if_statements)*
                Err(ProgramError::InvalidArgument.into())
            }
        }
    }
}

fn gen_instruction(idl: &Idl) -> proc_macro2::TokenStream {
    let variants = idl
        .instructions
        .iter()
        .map(|ix| format_ident!("{}", ix.name.to_camel_case())).map(
        |name| quote! { #name { accounts: client::accounts::#name, args: client::args::#name } },
    );
    let if_statements = {
        fn gen_accounts(
            name: &str,
            ix_accs: &[IdlInstructionAccountItem],
            all_ix_accs: &[IdlInstructionAccounts],
        ) -> proc_macro2::TokenStream {
            let name = format_ident!("{}", name.to_camel_case());
            let fields = ix_accs.iter().map(|acc| match acc {
                IdlInstructionAccountItem::Single(acc) => {
                    let name = format_ident!("{}", acc.name);
                    let signer = acc.signer;
                    let writable = acc.writable;
                    let optional = acc.optional;
                    if optional {
                        // For optional accounts, the program ID is used as a placeholder when missing
                        let program_id = get_canonical_program_id();
                        quote! {
                            #name: {
                                let acc = accs.next().ok_or_else(|| ProgramError::NotEnoughAccountKeys)?;
                                // Check if this is a placeholder (program_id used for missing optional accounts)
                                if acc.pubkey == #program_id {
                                    None
                                } else {
                                    if acc.is_signer != #signer {
                                        return Err(ProgramError::InvalidAccountData.into());
                                    }
                                    if acc.is_writable != #writable {
                                        return Err(ProgramError::InvalidAccountData.into());
                                    }
                                    Some(acc.pubkey)
                                }
                            }
                        }
                    } else {
                        quote! {
                            #name: {
                                let acc = accs.next().ok_or_else(|| ProgramError::NotEnoughAccountKeys)?;
                                if acc.is_signer != #signer {
                                    return Err(ProgramError::InvalidAccountData.into());
                                }
                                if acc.is_writable != #writable {
                                    return Err(ProgramError::InvalidAccountData.into());
                                }

                                acc.pubkey
                            }
                        }
                    }
                }
                IdlInstructionAccountItem::Composite(accs) => {
                    let name = format_ident!("{}", accs.name);
                    #[allow(clippy::expect_used, reason = "accounts are guaranteed to exist by prior deduplication pass")]
                    let accounts = all_ix_accs
                        .iter()
                        .find(|a| a.accounts == accs.accounts)
                        .map(|a| gen_accounts(&a.name, &a.accounts, all_ix_accs))
                        .expect("Accounts must exist");
                    quote! { #name: #accounts }
                }
            });

            quote! { client::accounts::#name { #(#fields,)* } }
        }

        let all_ix_accs = get_all_instruction_accounts(idl);
        idl.instructions
            .iter()
            .map(|ix| {
                let name = format_ident!("{}", ix.name.to_camel_case());
                let accounts = gen_accounts(&ix.name, &ix.accounts, &all_ix_accs);
                quote! {
                    if ix.data.starts_with(client::args::#name::DISCRIMINATOR) {
                        let mut accs = ix.accounts.to_owned().into_iter();
                        return Ok(Self::#name {
                            accounts: #accounts,
                            args: client::args::#name::try_from_slice(
                                &ix.data[client::args::#name::DISCRIMINATOR.len()..]
                            )?
                        })
                    }
                }
            })
            .collect::<Vec<_>>()
    };

    let solana_instruction = quote!(anchor_lang::solana_program::instruction::Instruction);
    let program_id = get_canonical_program_id();

    quote! {
        /// An enum that includes all instructions of the declared program.
        ///
        /// See [`Self::parse`] to create an instance from
        /// [`anchor_lang::solana_program::instruction::Instruction`].
        pub enum Instruction {
            #(#variants,)*
        }

        impl Instruction {
            ///  Parse an instruction based on the given
            /// [`anchor_lang::solana_program::instruction::Instruction`].
            ///
            /// This method checks:
            ///
            /// - The program ID
            /// - There is no missing account(s)
            /// - All accounts have the correct signer and writable attributes
            /// - The instruction data can be deserialized
            ///
            /// It does **not** check whether:
            ///
            /// - There are more accounts than expected
            /// - The account addresses match the ones that could be derived using the resolution
            ///   fields such as `address` and `pda`
            pub fn parse(ix: &#solana_instruction) -> Result<Self> {
                Self::try_from(ix)
            }
        }

        impl TryFrom<&#solana_instruction> for Instruction {
            type Error = anchor_lang::error::Error;

            fn try_from(ix: &#solana_instruction) -> Result<Self> {
                if ix.program_id != #program_id {
                    return Err(ProgramError::IncorrectProgramId.into())
                }

                #(#if_statements)*
                Err(ProgramError::InvalidInstructionData.into())
            }
        }
    }
}
