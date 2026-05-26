use {
    anchor_lang_v2::{Discriminator, Id, IdlAccountType, InstructionData},
    declare_program_types::weird_types,
    solana_pubkey::Pubkey,
};

fn weird_types_id() -> Pubkey {
    "Hy6xBbKVJBPx5PV7VsUqrJxGaRFgbmyJkYyzc1L6YMmU"
        .parse()
        .unwrap()
}

#[test]
fn declared_weird_types_exports_program_marker_and_id() {
    assert_eq!(weird_types::ID, weird_types_id());
    assert_eq!(weird_types::program::WeirdTypes::id(), weird_types_id());
}

#[test]
fn declared_weird_types_compile_and_serialize() {
    fn assert_idl_type<T: IdlAccountType>() {}

    assert_idl_type::<weird_types::UnitMarker>();
    assert_idl_type::<weird_types::EmptyNamed>();
    assert_idl_type::<weird_types::PairTuple>();
    assert_idl_type::<weird_types::GenericBox<u64>>();
    assert_idl_type::<weird_types::FixedBytes<4>>();
    assert_idl_type::<weird_types::NestedGeneric<u16, 4>>();
    assert_idl_type::<weird_types::WeirdEnum<u16, 4>>();
    assert_idl_type::<weird_types::DocumentedBorsh>();
    assert_idl_type::<weird_types::PackedBytemuck>();
    assert_idl_type::<weird_types::TransparentBytemuck>();
    assert_idl_type::<weird_types::AlignedBytemuck>();

    assert!(<weird_types::UnitMarker as IdlAccountType>::__IDL_TYPE_DEF
        .expect("unit struct should retain its IDL type definition")
        .contains("\"name\":\"UnitMarker\""));
    let documented_type_def = <weird_types::DocumentedBorsh as IdlAccountType>::__IDL_TYPE_DEF
        .expect("documented type should retain its IDL type definition");
    assert!(documented_type_def
        .contains("\"docs\":[\"A borsh type with docs and explicit repr metadata.\"]"));
    assert!(documented_type_def.contains("\"repr\":{\"kind\":\"c\"}"));

    let data = weird_types::instruction::UseWeirdTypes {
        float32: 1.5,
        float64: -2.25,
        unit: weird_types::UnitMarker,
        empty: weird_types::EmptyNamed {},
        tuple: weird_types::PairTuple(7, true),
        alias: 99,
        generic: weird_types::GenericBox { value: 123 },
        fixed: weird_types::FixedBytes { data: [1, 2, 3, 4] },
        nested: weird_types::NestedGeneric {
            boxed: weird_types::GenericBox { value: 55 },
            fixed: weird_types::FixedBytes { data: [5, 6, 7, 8] },
            items: vec![9, 10],
        },
        mode: weird_types::WeirdEnum::Named {
            fixed: [11, 12, 13, 14],
            boxed: weird_types::GenericBox { value: 15 },
        },
        boxed_alias: weird_types::GenericBox { value: 777 },
        documented: weird_types::DocumentedBorsh { value: 31337 },
    }
    .data();

    assert_eq!(
        weird_types::instruction::UseWeirdTypes::DISCRIMINATOR,
        &[90, 91, 92, 93]
    );
    assert_eq!(&data[..4], &[90, 91, 92, 93]);
    assert!(data.len() > 4);
}

#[test]
fn declared_weird_type_docs_and_repr_metadata_compile() {
    fn assert_pod<T: anchor_lang_v2::bytemuck::Pod>() {}

    assert_pod::<weird_types::PackedBytemuck>();
    assert_pod::<weird_types::TransparentBytemuck>();
    assert_pod::<weird_types::AlignedBytemuck>();

    assert_eq!(
        core::mem::size_of::<weird_types::PackedBytemuck>(),
        core::mem::size_of::<u64>() + core::mem::size_of::<u8>()
    );
    assert_eq!(core::mem::align_of::<weird_types::PackedBytemuck>(), 1);
    assert_eq!(core::mem::size_of::<weird_types::TransparentBytemuck>(), 8);
    assert_eq!(core::mem::align_of::<weird_types::AlignedBytemuck>(), 16);
    assert_eq!(core::mem::size_of::<weird_types::AlignedBytemuck>(), 16);

    let packed = weird_types::PackedBytemuck { wide: 7, tag: 9 };
    assert_eq!(anchor_lang_v2::bytemuck::bytes_of(&packed).len(), 9);

    let packed_type_def = <weird_types::PackedBytemuck as IdlAccountType>::__IDL_TYPE_DEF
        .expect("packed type should retain its IDL type definition");
    assert!(packed_type_def
        .contains("\"docs\":[\"A packed bytemuck type with no implicit repr(C) override.\"]"));
    assert!(packed_type_def.contains("\"repr\":{\"kind\":\"c\",\"packed\":true}"));
}
