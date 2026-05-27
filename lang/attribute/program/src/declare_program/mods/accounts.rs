use {
    super::common::{convert_idl_type_def_to_ts, gen_discriminator, get_canonical_program_id},
    anchor_lang_idl::types::{Idl, IdlSerialization},
    quote::{format_ident, quote},
};

pub fn gen_accounts_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let accounts = idl.accounts.iter().map(|acc| {
        let name = format_ident!("{}", acc.name);
        let discriminator = gen_discriminator(&acc.discriminator);
        let disc = quote! { #name::DISCRIMINATOR };

        #[allow(clippy::expect_used, reason = "IDL account types are guaranteed to exist in types array")]
        let ty_def = idl
            .types
            .iter()
            .find(|ty| ty.name == acc.name)
            .expect("Type must exist");

        let impls = {
            let try_deserialize = quote! {
                fn try_deserialize(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                    if buf.len() < #disc.len() {
                        return Err(anchor_lang::error::ErrorCode::AccountDiscriminatorNotFound.into());
                    }

                    let given_disc = &buf[..#disc.len()];
                    if #disc != given_disc {
                        return Err(
                            anchor_lang::error!(anchor_lang::error::ErrorCode::AccountDiscriminatorMismatch)
                            .with_account_name(stringify!(#name))
                        );
                    }

                    Self::try_deserialize_unchecked(buf)
                }
            };
            match ty_def.serialization {
                IdlSerialization::Borsh => quote! {
                    impl anchor_lang::AccountSerialize for #name {
                        fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> anchor_lang::Result<()> {
                            if writer.write_all(#disc).is_err() {
                                return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
                            }
                            if AnchorSerialize::serialize(self, writer).is_err() {
                                return Err(anchor_lang::error::ErrorCode::AccountDidNotSerialize.into());
                            }

                            Ok(())
                        }
                    }

                    impl anchor_lang::AccountDeserialize for #name {
                        #try_deserialize

                        fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                            let mut data: &[u8] = &buf[#disc.len()..];
                            AnchorDeserialize::deserialize(&mut data)
                                .map_err(|_| anchor_lang::error::ErrorCode::AccountDidNotDeserialize.into())
                        }
                    }
                },
                _ => {
                    let unsafe_bytemuck_impl =
                        matches!(ty_def.serialization, IdlSerialization::BytemuckUnsafe)
                            .then(|| {
                                quote! {
                                    unsafe impl anchor_lang::__private::bytemuck::Pod for #name {}
                                    unsafe impl anchor_lang::__private::bytemuck::Zeroable for #name {}
                                }
                            })
                            .unwrap_or_default();

                    quote! {
                        impl anchor_lang::ZeroCopy for #name {}

                        impl anchor_lang::AccountDeserialize for #name {
                            #try_deserialize

                            fn try_deserialize_unchecked(buf: &mut &[u8]) -> anchor_lang::Result<Self> {
                                let data: &[u8] = &buf[#disc.len()..];
                                Ok(anchor_lang::__private::bytemuck::pod_read_unaligned(data))
                            }
                        }

                        #unsafe_bytemuck_impl
                    }
                }
            }
        };

        let type_def_ts = convert_idl_type_def_to_ts(ty_def, &idl.types);
        let program_id = get_canonical_program_id();

        quote! {
            #type_def_ts

            #impls

            impl anchor_lang::Discriminator for #name {
                const DISCRIMINATOR: &'static [u8] = &#discriminator;
            }

            impl anchor_lang::Owner for #name {
                fn owner() -> Pubkey {
                    #program_id
                }
            }
        }
    });

    quote! {
        /// Program account type definitions.
        pub mod accounts {
            use super::*;

            #(#accounts)*
        }
    }
}
