use {
    anchor_lang::{AccountDeserialize, Discriminator, Id, InstructionData, BORSH_CONFIG},
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
        T: anchor_lang::Owner
            + anchor_lang::Discriminator
            + anchor_lang::AccountDeserialize
            + anchor_lang::IdlAccountType
            + anchor_lang::wincode::SchemaWrite<anchor_lang::BorshConfig, Src = T>
            + for<'de> anchor_lang::wincode::SchemaRead<'de, anchor_lang::BorshConfig, Dst = T>,
    {
    }

    fn assert_borsh_account_wrapper_idl<T>()
    where
        T: anchor_lang::Owner
            + anchor_lang::Discriminator
            + anchor_lang::wincode::SchemaWrite<anchor_lang::BorshConfig, Src = T>
            + for<'de> anchor_lang::wincode::SchemaRead<'de, anchor_lang::BorshConfig, Dst = T>,
        anchor_lang::accounts::BorshAccount<T>: anchor_lang::IdlAccountType,
    {
    }

    fn assert_zero_copy_account<T>()
    where
        T: anchor_lang::Owner
            + anchor_lang::Discriminator
            + anchor_lang::accounts::SlabSchema
            + anchor_lang::bytemuck::Pod
            + anchor_lang::bytemuck::Zeroable,
    {
    }

    assert_borsh_account::<serialization::ImplicitBorshAccount>();
    assert_borsh_account::<serialization::ExplicitBorshAccount>();
    assert_borsh_account_wrapper_idl::<serialization::ImplicitBorshAccount>();
    assert_borsh_account_wrapper_idl::<serialization::ExplicitBorshAccount>();
    assert_zero_copy_account::<serialization::ZeroCopyAccount>();
    assert_zero_copy_account::<serialization::UnsafeZeroCopyAccount>();
    assert_zero_copy_account::<serialization::PaddedUnsafeAccount>();

    fn assert_pod<T: anchor_lang::bytemuck::Pod + anchor_lang::bytemuck::Zeroable>() {}
    assert_pod::<serialization::PackedUnsafeAccount>();

    assert!(
        <serialization::ImplicitBorshAccount as anchor_lang::IdlAccountType>::__IDL_ACCOUNT_ENTRY
            .expect("declared account type should have an IDL account entry")
            .contains("\"name\":\"ImplicitBorshAccount\"")
    );
    assert!(
        <serialization::ImplicitBorshAccount as anchor_lang::IdlAccountType>::__IDL_TYPE_DEF
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
        <serialization::PaddedUnsafeAccount as Discriminator>::DISCRIMINATOR,
        &[51, 52, 53, 54, 55, 56, 57, 58]
    );

    assert_eq!(
        <serialization::ImplicitBorshAccount as anchor_lang::Owner>::OWNER,
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
    let zero_bytes = anchor_lang::bytemuck::bytes_of(&zero);
    assert_eq!(zero_bytes.len(), 16);
    assert_eq!(&zero_bytes[..8], &0x0102_0304_0506_0708u64.to_le_bytes());
    assert_eq!(&zero_bytes[8..12], &0x1112_1314u32.to_le_bytes());
    assert_eq!(&zero_bytes[12..16], b"zero");

    // `bytemuckunsafe` must accept layouts that fail safe Pod checks:
    // non-Pod `bool` plus repr(C) padding after it, and the v1 default
    // packed layout when the IDL omits repr.
    assert_eq!(core::mem::size_of::<serialization::PaddedUnsafeAccount>(), 16);
    assert_eq!(core::mem::align_of::<serialization::PaddedUnsafeAccount>(), 8);
    let mut padded_bytes = [0u8; 16];
    padded_bytes[0] = 1;
    padded_bytes[8..16].copy_from_slice(&0x0102_0304_0506_0708u64.to_le_bytes());
    let padded: serialization::PaddedUnsafeAccount =
        anchor_lang::bytemuck::pod_read_unaligned(&padded_bytes);
    assert!(padded.flag);
    assert_eq!(padded.wide, 0x0102_0304_0506_0708);

    assert_eq!(core::mem::size_of::<serialization::PackedUnsafeAccount>(), 9);
    assert_eq!(core::mem::align_of::<serialization::PackedUnsafeAccount>(), 1);
    let mut packed_bytes = [0u8; 9];
    packed_bytes[0] = 7;
    packed_bytes[1..9].copy_from_slice(&99u64.to_le_bytes());
    let packed: serialization::PackedUnsafeAccount =
        anchor_lang::bytemuck::pod_read_unaligned(&packed_bytes);
    assert_eq!(packed.tag, 7);
    let packed_wide = packed.wide;
    assert_eq!(packed_wide, 99);
}

#[test]
fn declared_account_deserialize_unchecked_skips_discriminator_prefix() {
    let original = serialization::ImplicitBorshAccount {
        count: 9,
        label: "decoded".to_string(),
        items: vec![5, 6, 7],
    };
    let payload =
        anchor_lang::wincode::config::serialize(&original, BORSH_CONFIG).unwrap();
    let mut bytes = Vec::from(serialization::ImplicitBorshAccount::DISCRIMINATOR);
    bytes.extend_from_slice(&payload);

    let mut buf = bytes.as_slice();
    let decoded = serialization::ImplicitBorshAccount::try_deserialize_unchecked(&mut buf)
        .expect("full account bytes should deserialize without checking the discriminator");

    assert_eq!(decoded.count, original.count);
    assert_eq!(decoded.label, original.label);
    assert_eq!(decoded.items, original.items);
    assert!(buf.is_empty(), "decoder should consume the full account buffer");
}

#[test]
fn declared_account_deserialize_rejects_wrong_discriminator() {
    let original = serialization::ImplicitBorshAccount {
        count: 1,
        label: "wrong-disc".to_string(),
        items: vec![8, 9],
    };
    let payload =
        anchor_lang::wincode::config::serialize(&original, BORSH_CONFIG).unwrap();
    let mut bytes = vec![0u8; serialization::ImplicitBorshAccount::DISCRIMINATOR.len()];
    bytes.extend_from_slice(&payload);

    let mut buf = bytes.as_slice();
    assert!(
        serialization::ImplicitBorshAccount::try_deserialize(&mut buf).is_err(),
        "checked deserialize must reject a wrong discriminator"
    );
}
