#![cfg(feature = "idl-build")]

use {
    anchor_syn::idl::impl_idl_build_struct,
    syn::{parse_quote, ItemStruct},
};

#[test]
fn qualified_builtin_paths_lower_to_builtin_idl_types() {
    let item: ItemStruct = parse_quote! {
        struct QualifiedBuiltins {
            pubkeys: ::std::vec::Vec<anchor_lang::prelude::Pubkey>,
            maybe: ::core::option::Option<u8>,
            label: ::std::string::String,
        }
    };

    let output = impl_idl_build_struct(&item).to_string();

    for expected in [
        "IdlType :: Vec (Box :: new (anchor_lang :: idl :: types :: IdlType :: Pubkey))",
        "IdlType :: Option (Box :: new (anchor_lang :: idl :: types :: IdlType :: U8))",
        "IdlType :: String",
    ] {
        assert!(
            output.contains(expected),
            "Output did not contain expected IDL type fragment: '{expected}'. Got: '{output}'",
        );
    }

    for unexpected in [
        "< :: std :: vec :: Vec < anchor_lang :: prelude :: Pubkey > > :: get_full_path",
        "< :: core :: option :: Option < u8 > > :: get_full_path",
        "< :: std :: string :: String > :: get_full_path",
        "< anchor_lang :: prelude :: Pubkey > :: get_full_path",
    ] {
        assert!(
            !output.contains(unexpected),
            "Output incorrectly treated a qualified builtin as a defined type: '{unexpected}'. \
             Got: '{output}'",
        );
    }
}

#[test]
fn additional_qualified_builtin_paths_lower_to_builtin_idl_types() {
    let item: ItemStruct = parse_quote! {
        struct QualifiedBuiltins {
            alloc_string: alloc::string::String,
            crate_pubkey: solana_pubkey::Pubkey,
            program_pubkey: solana_program::pubkey::Pubkey,
            reexported_pubkey: anchor_lang::solana_program::pubkey::Pubkey,
            nested_string: Vec<alloc::string::String>,
            nested_pubkey: Option<solana_pubkey::Pubkey>,
        }
    };

    let output = impl_idl_build_struct(&item).to_string();

    for expected in [
        "IdlType :: String",
        "IdlType :: Pubkey",
        "IdlType :: Vec (Box :: new (anchor_lang :: idl :: types :: IdlType :: String))",
        "IdlType :: Option (Box :: new (anchor_lang :: idl :: types :: IdlType :: Pubkey))",
    ] {
        assert!(
            output.contains(expected),
            "Output did not contain expected IDL type fragment: '{expected}'. Got: '{output}'",
        );
    }

    // `Self :: get_full_path` is emitted for the struct itself; a defined field
    // type is emitted as `< T > :: get_full_path`.
    assert!(
        !output.contains("> :: get_full_path"),
        "Output treated a qualified builtin as a defined type. Got: '{output}'",
    );
}

#[test]
fn user_defined_types_named_like_builtins_stay_defined() {
    let item: ItemStruct = parse_quote! {
        struct UserTypes {
            label: my_mod::String,
            key: my_mod::Pubkey,
        }
    };

    let output = impl_idl_build_struct(&item).to_string();

    for expected in [
        "< my_mod :: String > :: get_full_path",
        "< my_mod :: Pubkey > :: get_full_path",
    ] {
        assert!(
            output.contains(expected),
            "Output did not treat a user-defined type as a defined type: '{expected}'. Got: \
             '{output}'",
        );
    }
}
