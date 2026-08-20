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
