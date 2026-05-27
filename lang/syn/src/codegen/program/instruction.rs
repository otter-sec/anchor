use {
    crate::{codegen::program::common::*, parser, Program},
    heck::CamelCase,
    quote::{quote, quote_spanned, ToTokens},
    syn::Type,
};

/// Returns true for primitives, common std types, and types wrapped in Option/Vec.
fn can_derive_common_trait(ty: &Type) -> bool {
    match ty {
        // Primitives - always support Clone/Debug
        Type::Path(path) if path.qself.is_none() => {
            let Some(last_segment) = path.path.segments.last() else {
                return false;
            };
            let ident_str = last_segment.ident.to_string();

            // Check for primitives
            if matches!(
                ident_str.as_str(),
                "bool"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "char"
                    | "str"
            ) {
                return true;
            }

            // For Option<T> and Vec<T>, check the inner type first
            if ident_str == "Option" || ident_str == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return can_derive_common_trait(inner_ty);
                    }
                }
                // If we can't extract the inner type, Vec/Option themselves support Clone/Debug
                return true;
            }

            // Check for common std types that support Clone/Debug
            if matches!(ident_str.as_str(), "String" | "Pubkey") {
                return true;
            }

            // For user-defined types, we can't verify at macro time
            false
        }
        Type::Array(arr) => can_derive_common_trait(&arr.elem),
        Type::Tuple(tuple) => tuple.elems.iter().all(can_derive_common_trait),
        Type::Reference(reference) => can_derive_common_trait(&reference.elem),
        // For other types, be conservative
        _ => false,
    }
}

pub fn generate(program: &Program) -> proc_macro2::TokenStream {
    let variants: Vec<proc_macro2::TokenStream> = program
        .ixs
        .iter()
        .map(|ix| {
            let name = &ix.raw_method.sig.ident.to_string();
            let ix_cfgs = &ix.cfgs;
            let Ok(ix_name_camel) = syn::parse_str::<syn::Ident>(&name.to_camel_case()) else {
                return quote_spanned! { ix.raw_method.sig.ident.span()=>
                    compile_error!("failed to parse ix method name after conversion to camelCase");
                };
            };
            let raw_args: Vec<proc_macro2::TokenStream> = ix
                .args
                .iter()
                .map(|arg| {
                    #[allow(
                        clippy::unwrap_used,
                        reason = "\"pub \" prepended to a valid field token string is always \
                                  valid Rust"
                    )]
                    let ts = format!("pub {}", parser::tts_to_string(&arg.raw_arg))
                        .parse()
                        .unwrap();
                    ts
                })
                .collect();

            // Conditionally derive Clone and Debug if every arg type supports them.
            // `all()` is true for empty iterators, so unit instructions always get the derives.
            let extra_derives: Vec<proc_macro2::TokenStream> = if ix
                .args
                .iter()
                .all(|arg| can_derive_common_trait(&arg.raw_arg.ty))
            {
                vec![quote!(Clone), quote!(Debug)]
            } else {
                vec![]
            };

            let impls = {
                let discriminator = match ix.overrides.as_ref() {
                    Some(overrides) if overrides.discriminator.is_some() => {
                        overrides.discriminator.to_token_stream()
                    }
                    _ => gen_discriminator(SIGHASH_GLOBAL_NAMESPACE, name),
                };

                quote! {
                    #(#ix_cfgs)*
                    impl anchor_lang::Discriminator for #ix_name_camel {
                        const DISCRIMINATOR: &'static [u8] = #discriminator;
                    }
                    #(#ix_cfgs)*
                    impl anchor_lang::InstructionData for #ix_name_camel {}
                    #(#ix_cfgs)*
                    impl anchor_lang::Owner for #ix_name_camel {
                        fn owner() -> Pubkey {
                            ID
                        }
                    }
                }
            };
            // If no args, output a "unit" variant instead of a struct variant.
            if ix.args.is_empty() {
                quote! {
                    #(#ix_cfgs)*
                    /// Instruction.
                    #[derive(AnchorSerialize, AnchorDeserialize #(, #extra_derives)*)]
                    pub struct #ix_name_camel;

                    #impls
                }
            } else {
                quote! {
                    #(#ix_cfgs)*
                    /// Instruction.
                    #[derive(AnchorSerialize, AnchorDeserialize #(, #extra_derives)*)]
                    pub struct #ix_name_camel {
                        #(#raw_args),*
                    }

                    #impls
                }
            }
        })
        .collect();

    quote! {
        /// An Anchor generated module containing the program's set of
        /// instructions, where each method handler in the `#[program]` mod is
        /// associated with a struct defining the input arguments to the
        /// method. These should be used directly, when one wants to serialize
        /// Anchor instruction data, for example, when specifying
        /// instructions on a client.
        pub mod instruction {
            use super::*;

            #(#variants)*
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::can_derive_common_trait, syn::parse_quote};

    #[test]
    fn primitives_and_std_types_derive() {
        assert!(can_derive_common_trait(&parse_quote!(u8)));
        assert!(can_derive_common_trait(&parse_quote!(i128)));
        assert!(can_derive_common_trait(&parse_quote!(bool)));
        assert!(can_derive_common_trait(&parse_quote!(String)));
        assert!(can_derive_common_trait(&parse_quote!(Pubkey)));
    }

    #[test]
    fn fully_qualified_paths_resolve_to_last_segment() {
        // `std::vec::Vec<u8>` should be recognised as a `Vec` of a primitive.
        assert!(can_derive_common_trait(&parse_quote!(std::vec::Vec<u8>)));
        assert!(can_derive_common_trait(&parse_quote!(
            ::std::option::Option<u64>
        )));
    }

    #[test]
    fn option_and_vec_recurse_into_inner() {
        assert!(can_derive_common_trait(&parse_quote!(Option<u64>)));
        assert!(can_derive_common_trait(&parse_quote!(Vec<String>)));
        // Inner user-defined type makes the wrapper undecidable.
        assert!(!can_derive_common_trait(&parse_quote!(Option<MyType>)));
        assert!(!can_derive_common_trait(&parse_quote!(Vec<MyType>)));
    }

    #[test]
    fn arrays_tuples_and_references_recurse() {
        assert!(can_derive_common_trait(&parse_quote!([u8; 32])));
        assert!(can_derive_common_trait(&parse_quote!((u8, bool, String))));
        assert!(can_derive_common_trait(&parse_quote!(&u8)));

        assert!(!can_derive_common_trait(&parse_quote!([MyType; 4])));
        assert!(!can_derive_common_trait(&parse_quote!((u8, MyType))));
    }

    #[test]
    fn user_defined_types_are_conservative() {
        // Without name resolution we can't know whether `MyType` is Clone/Debug.
        assert!(!can_derive_common_trait(&parse_quote!(MyType)));
        assert!(!can_derive_common_trait(&parse_quote!(some_mod::MyType)));
    }

    #[test]
    fn exotic_types_rejected() {
        // Trait objects, fn pointers, impl traits — none are supported.
        assert!(!can_derive_common_trait(&parse_quote!(dyn std::fmt::Debug)));
        assert!(!can_derive_common_trait(&parse_quote!(fn(u8) -> u8)));
    }
}
