use {crate::Program, heck::CamelCase, quote::quote};

pub fn generate(program: &Program) -> proc_macro2::TokenStream {
    // Dispatch all global instructions.
    let global_ixs = program.ixs.iter().map(|ix| {
        let ix_method_name = &ix.raw_method.sig.ident;
        #[allow(
            clippy::expect_used,
            reason = "camelCase of a valid Rust identifier is always a valid TokenStream"
        )]
        let ix_name_camel: proc_macro2::TokenStream = ix_method_name
            .to_string()
            .to_camel_case()
            .parse()
            .expect("Failed to parse ix method name in camel as `TokenStream`");
        let discriminator = quote! { instruction::#ix_name_camel::DISCRIMINATOR };
        let ix_cfgs = &ix.cfgs;

        quote! {
            #(#ix_cfgs)*
            if data.starts_with(#discriminator) {
                return __private::__global::#ix_method_name(
                    program_id,
                    accounts,
                    &data[#discriminator.len()..],
                )
            }
        }
    });

    // Generate the event-cpi instruction handler based on whether the `event-cpi` feature is enabled.
    let event_cpi_handler = {
        #[cfg(feature = "event-cpi")]
        quote! {
            // `event-cpi` feature is enabled, dispatch self-cpi instruction
            __private::__events::__event_dispatch(
                program_id,
                accounts,
                &data[anchor_lang::event::EVENT_IX_TAG_LE.len()..]
            )
        }
        #[cfg(not(feature = "event-cpi"))]
        quote! {
            // `event-cpi` feature is not enabled
            Err(anchor_lang::error::ErrorCode::EventInstructionStub.into())
        }
    };

    let fallback_fn = program
        .fallback_fn
        .as_ref()
        .map(|fallback_fn| {
            let program_name = &program.name;
            let fn_name = &fallback_fn.raw_method.sig.ident;
            quote! {
                #program_name::#fn_name(program_id, accounts, data)
            }
        })
        .unwrap_or_else(|| {
            quote! {
                Err(anchor_lang::error::ErrorCode::InstructionFallbackNotFound.into())
            }
        });

    let legacy_idl_dispatch = match &program.program_args {
        Some(args) if args.legacy_idl => generate_legacy_idl_dispatch(),
        _ => proc_macro2::TokenStream::new(),
    };

    quote! {
        /// Performs method dispatch.
        ///
        /// Each instruction's discriminator is checked until the given instruction data starts with
        /// the current discriminator.
        ///
        /// If a match is found, the instruction handler is called using the given instruction data
        /// excluding the prepended discriminator bytes.
        ///
        /// If no match is found, the fallback function is executed if it exists, or an error is
        /// returned if it doesn't exist.
        fn dispatch<'info>(
            program_id: &'info Pubkey,
            accounts: &'info [AccountInfo<'info>],
            data: &'info [u8],
        ) -> anchor_lang::Result<()> {
            #(#global_ixs)*

            #legacy_idl_dispatch

            // Dispatch Event CPI instruction
            if data.starts_with(anchor_lang::event::EVENT_IX_TAG_LE) {
                return #event_cpi_handler;
            }

            #fallback_fn
        }
    }
}

fn generate_legacy_idl_dispatch() -> proc_macro2::TokenStream {
    quote! {

        ::anchor_lang::idl::deprecated_legacy_idl_usage();

        // dispatch legacy IDL instructions when requested by `#[program(legacy_idl)]`
        if data.starts_with(anchor_lang::idl::IDL_IX_TAG_LE) {
            return __private::__idl::__idl_dispatch(
                program_id,
                accounts,
                &data[anchor_lang::idl::IDL_IX_TAG_LE.len()..],
            );
        }
    }
}
