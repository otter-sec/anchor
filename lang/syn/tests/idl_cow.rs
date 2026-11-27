#![cfg(feature = "idl-build")]

use {
    anchor_syn::idl::impl_idl_build_struct,
    syn::{parse_quote, ItemStruct},
};

#[test]
fn cow_lowers_to_inner_idl_types() {
    let item: ItemStruct = parse_quote! {
        struct Payload<'a> {
            a: Cow<'a, [u8]>,
            b: std::borrow::Cow<'a, [u8]>,
            c: ::alloc::borrow::Cow<'static, [u8]>,
            d: Option<Cow<'a, [u8]>>,
            e: Cow<'a, str>,
        }
    };

    let output = impl_idl_build_struct(&item).to_string();
    let idl_type = "anchor_lang :: idl :: types :: IdlType";

    for (field, ty) in [
        ("a", format!("{idl_type} :: Bytes")),
        ("b", format!("{idl_type} :: Bytes")),
        ("c", format!("{idl_type} :: Bytes")),
        (
            "d",
            format!("{idl_type} :: Option (Box :: new ({idl_type} :: Bytes))"),
        ),
        ("e", format!("{idl_type} :: String")),
    ] {
        let expected = format!("name : \"{field}\" . into () , docs : vec ! [] , ty : {ty}");
        assert!(
            output.contains(&expected),
            "Output did not contain expected IDL field: '{expected}'. Got: '{output}'",
        );
    }

    assert!(
        !output.contains("Cow"),
        "Output still references a Cow type, so it was treated as a defined type. Got: '{output}'",
    );
}

#[test]
fn defined_type_lifetime_args_are_skipped() {
    let item: ItemStruct = parse_quote! {
        struct Outer<'a> {
            payload: Payload<'a>,
        }
    };

    let output = impl_idl_build_struct(&item).to_string();

    assert!(
        !output.contains("compile_error"),
        "Output contains a compile error. Got: '{output}'",
    );
    assert!(
        output.contains("name : < Payload < 'a > > :: get_full_path () , generics : vec ! [] ,"),
        "Output did not lower `Payload<'a>` to a defined type without generics. Got: '{output}'",
    );
}
