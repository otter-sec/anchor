use {
    super::common::{convert_idl_type_def_to_ts, gen_discriminator},
    anchor_lang_idl::types::Idl,
    quote::{format_ident, quote},
};

pub fn gen_events_mod(idl: &Idl) -> proc_macro2::TokenStream {
    let events = idl.events.iter().map(|ev| {
        let name = format_ident!("{}", ev.name);
        let discriminator = gen_discriminator(&ev.discriminator);

        #[allow(
            clippy::expect_used,
            reason = "IDL event types are guaranteed to exist in types array"
        )]
        let ty_def = idl
            .types
            .iter()
            .find(|ty| ty.name == ev.name)
            .map(|ty| convert_idl_type_def_to_ts(ty, &idl.types))
            .expect("Type must exist");

        quote! {
            #ty_def

            impl anchor_lang::Event for #name {
                fn data(&self) -> Vec<u8> {
                    let mut data = Vec::with_capacity(256);
                    data.extend_from_slice(#name::DISCRIMINATOR);
                    self.serialize(&mut data).unwrap();
                    data
                }
            }

            impl anchor_lang::Discriminator for #name {
                const DISCRIMINATOR: &'static [u8] = &#discriminator;
            }
        }
    });

    quote! {
        /// Program event type definitions.
        pub mod events {
            use super::*;

            #(#events)*
        }
    }
}
