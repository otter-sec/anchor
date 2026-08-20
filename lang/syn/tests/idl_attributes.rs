#![cfg(feature = "idl-build")]

use {
    anchor_syn::idl::impl_idl_build_struct,
    syn::{parse_quote, ItemStruct},
};

fn serialization_expr(item: ItemStruct) -> String {
    let stream = impl_idl_build_struct(&item);
    let output = stream.to_string();

    let needle = "IdlSerialization :: ";
    let start = match output.find(needle) {
        Some(start) => start,
        None => unreachable!("Output did not contain serialization marker. Got: '{output}'"),
    };
    let rest = &output[start + needle.len()..];
    let end = match rest.find(',') {
        Some(end) => end,
        None => unreachable!("Output did not terminate serialization expression. Got: '{output}'"),
    };

    rest[..end].trim().to_owned()
}

fn check_serialization(item: ItemStruct, expected: &str) {
    let actual = serialization_expr(item);
    assert_eq!(actual, expected, "Unexpected serialization expression");
}

#[test]
fn test_bytemuck_unsafe_qualified() {
    check_serialization(
        parse_quote! {
            #[derive(bytemuck::Unsafe)]
            struct Foo {}
        },
        "BytemuckUnsafe",
    );
}

#[test]
fn test_bytemuck_safe() {
    check_serialization(
        parse_quote! {
            #[derive(bytemuck::Pod)]
            struct Foo {}
        },
        "Bytemuck",
    );
}

#[test]
fn test_bytemuck_non_pod_ignored() {
    check_serialization(
        parse_quote! {
            #[derive(bytemuck::AnyBitPattern)]
            struct Foo {}
        },
        "default ()",
    );
}

#[test]
fn test_false_positive_prevention() {
    check_serialization(
        parse_quote! {
            #[derive(MyUnsafeMacro)]
            struct Foo {}
        },
        "default ()",
    );
}

#[test]
fn test_non_exact_bytemuck_unsafe_ignored() {
    check_serialization(
        parse_quote! {
            #[derive(bytemuck::NotUnsafe)]
            struct Foo {}
        },
        "default ()",
    );
}

#[test]
fn test_nested_bytemuck_path_ignored() {
    check_serialization(
        parse_quote! {
            #[derive(foo::bytemuck::Pod)]
            struct Foo {}
        },
        "default ()",
    );
}

#[test]
fn test_invalid_derive_meta_surfaces() {
    let item = syn::parse_str::<ItemStruct>(
        r#"
        #[derive(bytemuck::Pod())]
        struct Foo {}
        "#,
    )
    .unwrap();
    let output = impl_idl_build_struct(&item).to_string();

    assert!(
        output.contains("compile_error !"),
        "Output did not contain compile_error for invalid derive metadata. Got: '{output}'",
    );
}

#[test]
fn test_bytemuck_safe_then_unsafe() {
    check_serialization(
        parse_quote! {
            #[derive(bytemuck::Pod, bytemuck::Unsafe)]
            struct Foo {}
        },
        "BytemuckUnsafe",
    );
}
