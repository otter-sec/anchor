use {
    anchor_lang_v2::{Discriminator, Id, InstructionData},
    declare_program_serialization::serialization,
    solana_pubkey::Pubkey,
};

fn serialization_id() -> Pubkey {
    "4wBqpZM9xaSheZzJSMawUKKwhdpChKbZ5eu5ky4Vigw"
        .parse()
        .unwrap()
}

#[test]
fn declared_serialization_exports_program_marker_and_id() {
    assert_eq!(serialization::ID, serialization_id());
    assert_eq!(
        serialization::program::Serialization::id(),
        serialization_id()
    );
}

#[test]
fn declared_program_type_serialization_controls_account_traits() {
    fn assert_borsh_account<T>()
    where
        T: anchor_lang_v2::Owner
            + anchor_lang_v2::Discriminator
            + anchor_lang_v2::IdlAccountType
            + anchor_lang_v2::wincode::SchemaWrite<anchor_lang_v2::BorshConfig, Src = T>
            + for<'de> anchor_lang_v2::wincode::SchemaRead<'de, anchor_lang_v2::BorshConfig, Dst = T>,
    {
    }

    fn assert_borsh_account_wrapper_idl<T>()
    where
        T: anchor_lang_v2::Owner
            + anchor_lang_v2::Discriminator
            + anchor_lang_v2::wincode::SchemaWrite<anchor_lang_v2::BorshConfig, Src = T>
            + for<'de> anchor_lang_v2::wincode::SchemaRead<'de, anchor_lang_v2::BorshConfig, Dst = T>,
        anchor_lang_v2::accounts::BorshAccount<T>: anchor_lang_v2::IdlAccountType,
    {
    }

    fn assert_zero_copy_account<T>()
    where
        T: anchor_lang_v2::Owner
            + anchor_lang_v2::Discriminator
            + anchor_lang_v2::accounts::SlabSchema
            + anchor_lang_v2::bytemuck::Pod
            + anchor_lang_v2::bytemuck::Zeroable,
    {
    }

    assert_borsh_account::<serialization::ImplicitBorshAccount>();
    assert_borsh_account::<serialization::ExplicitBorshAccount>();
    assert_borsh_account_wrapper_idl::<serialization::ImplicitBorshAccount>();
    assert_borsh_account_wrapper_idl::<serialization::ExplicitBorshAccount>();
    assert_zero_copy_account::<serialization::ZeroCopyAccount>();
    assert_zero_copy_account::<serialization::UnsafeZeroCopyAccount>();

    assert!(
        <serialization::ImplicitBorshAccount as anchor_lang_v2::IdlAccountType>::__IDL_ACCOUNT_ENTRY
            .expect("declared account type should have an IDL account entry")
            .contains("\"name\":\"ImplicitBorshAccount\"")
    );
    assert!(
        <serialization::ImplicitBorshAccount as anchor_lang_v2::IdlAccountType>::__IDL_TYPE_DEF
            .expect("declared account type should have an IDL type definition")
            .contains("\"name\":\"ImplicitBorshAccount\"")
    );

    assert_eq!(
        <serialization::ImplicitBorshAccount as Discriminator>::DISCRIMINATOR,
        &[11, 12, 13, 14, 15, 16, 17, 18]
    );
    assert_eq!(
        <serialization::ExplicitBorshAccount as Discriminator>::DISCRIMINATOR,
        &[21, 22, 23, 24, 25, 26, 27, 28]
    );
    assert_eq!(
        <serialization::ZeroCopyAccount as Discriminator>::DISCRIMINATOR,
        &[31, 32, 33, 34, 35, 36, 37, 38]
    );
    assert_eq!(
        <serialization::UnsafeZeroCopyAccount as Discriminator>::DISCRIMINATOR,
        &[41, 42, 43, 44, 45, 46, 47, 48]
    );

    assert_eq!(
        <serialization::ImplicitBorshAccount as anchor_lang_v2::Owner>::OWNER,
        serialization::ID
    );

    let implicit = serialization::instruction::UseImplicit {
        data: serialization::ImplicitBorshAccount {
            count: 7,
            label: "implicit".to_string(),
            items: vec![1, 2, 3],
        },
    }
    .data();
    assert_eq!(&implicit[..3], &[1, 2, 3]);
    assert!(implicit.len() > 3);

    let explicit = serialization::instruction::UseExplicit {
        data: serialization::ExplicitBorshAccount {
            enabled: true,
            fixed: *b"bors",
        },
    }
    .data();
    assert_eq!(&explicit[..3], &[4, 5, 6]);
    assert!(explicit.len() > 3);

    let zero = serialization::ZeroCopyAccount {
        wide: 0x0102_0304_0506_0708,
        narrow: 0x1112_1314,
        tag: *b"zero",
    };
    let zero_bytes = anchor_lang_v2::bytemuck::bytes_of(&zero);
    assert_eq!(zero_bytes.len(), 16);
    assert_eq!(&zero_bytes[..8], &0x0102_0304_0506_0708u64.to_le_bytes());
    assert_eq!(&zero_bytes[8..12], &0x1112_1314u32.to_le_bytes());
    assert_eq!(&zero_bytes[12..16], b"zero");
}
