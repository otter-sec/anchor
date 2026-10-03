#![cfg(feature = "idl-build")]

use {
    anchor_syn::idl::impl_idl_build_struct,
    syn::{parse_quote, ItemStruct},
};

#[test]
fn nonzero_integers_lower_to_inner_integer_idl_types() {
    let item: ItemStruct = parse_quote! {
        struct Limits {
            a: NonZeroU8,
            b: std::num::NonZeroU64,
            c: ::core::num::NonZeroI32,
            d: NonZero<u128>,
            e: std::num::NonZero<i16>,
            f: Option<NonZeroU32>,
            g: Vec<NonZeroI64>,
        }
    };

    let output = impl_idl_build_struct(&item).to_string();
    let idl_type = "anchor_lang :: idl :: types :: IdlType";

    for (field, ty) in [
        ("a", format!("{idl_type} :: U8")),
        ("b", format!("{idl_type} :: U64")),
        ("c", format!("{idl_type} :: I32")),
        ("d", format!("{idl_type} :: U128")),
        ("e", format!("{idl_type} :: I16")),
        (
            "f",
            format!("{idl_type} :: Option (Box :: new ({idl_type} :: U32))"),
        ),
        (
            "g",
            format!("{idl_type} :: Vec (Box :: new ({idl_type} :: I64))"),
        ),
    ] {
        let expected = format!("name : \"{field}\" . into () , docs : vec ! [] , ty : {ty}");
        assert!(
            output.contains(&expected),
            "Output did not contain expected IDL field: '{expected}'. Got: '{output}'",
        );
    }

    assert!(
        !output.contains("NonZero"),
        "Output still references a NonZero type, so it was treated as a defined type. Got: \
         '{output}'",
    );
}
