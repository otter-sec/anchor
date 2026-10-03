use anchor_lang::prelude::*;

// Test that Space trait is implemented for primitive types
#[test]
fn test_primitive_space_implementations() {
    assert_eq!(bool::INIT_SPACE, 1);
    assert_eq!(u8::INIT_SPACE, 1);
    assert_eq!(u16::INIT_SPACE, 2);
    assert_eq!(u32::INIT_SPACE, 4);
    assert_eq!(u64::INIT_SPACE, 8);
    assert_eq!(u128::INIT_SPACE, 16);
    assert_eq!(i8::INIT_SPACE, 1);
    assert_eq!(i16::INIT_SPACE, 2);
    assert_eq!(i32::INIT_SPACE, 4);
    assert_eq!(i64::INIT_SPACE, 8);
    assert_eq!(i128::INIT_SPACE, 16);
    assert_eq!(f32::INIT_SPACE, 4);
    assert_eq!(f64::INIT_SPACE, 8);
    assert_eq!(Pubkey::INIT_SPACE, 32);
}

// `NonZero*` integers are borsh-serialized exactly like the inner integer
#[test]
fn test_non_zero_space_implementations() {
    use std::num::*;

    assert_eq!(NonZeroU8::INIT_SPACE, 1);
    assert_eq!(NonZeroU16::INIT_SPACE, 2);
    assert_eq!(NonZeroU32::INIT_SPACE, 4);
    assert_eq!(NonZeroU64::INIT_SPACE, 8);
    assert_eq!(NonZeroU128::INIT_SPACE, 16);
    assert_eq!(NonZeroI8::INIT_SPACE, 1);
    assert_eq!(NonZeroI16::INIT_SPACE, 2);
    assert_eq!(NonZeroI32::INIT_SPACE, 4);
    assert_eq!(NonZeroI64::INIT_SPACE, 8);
    assert_eq!(NonZeroI128::INIT_SPACE, 16);
}

#[test]
fn test_non_zero_with_initspace() {
    use std::num::{NonZero, NonZeroU32, NonZeroU64};

    #[derive(InitSpace)]
    #[allow(dead_code)]
    struct Limits {
        max_supply: NonZeroU64,          // 8
        fee_bps: core::num::NonZeroU16,  // 2
        cap: NonZero<i128>,              // 16
        maybe_limit: Option<NonZeroU32>, // 1 + 4
        #[max_len(3)]
        tiers: Vec<NonZeroU64>, // 4 + 3 * 8
    }

    // Should be 8 + 2 + 16 + 5 + 28 = 59
    assert_eq!(Limits::INIT_SPACE, 59);

    let limits = Limits {
        max_supply: NonZeroU64::new(1).unwrap(),
        fee_bps: core::num::NonZeroU16::new(30).unwrap(),
        cap: NonZero::new(-1).unwrap(),
        maybe_limit: NonZeroU32::new(9),
        tiers: vec![NonZeroU64::MAX; 3],
    };
    let mut data = Vec::new();
    limits.max_supply.serialize(&mut data).unwrap();
    limits.fee_bps.serialize(&mut data).unwrap();
    limits.cap.serialize(&mut data).unwrap();
    limits.maybe_limit.serialize(&mut data).unwrap();
    limits.tiers.serialize(&mut data).unwrap();
    assert_eq!(data.len(), Limits::INIT_SPACE);
}

// Test that type aliases work with InitSpace
#[test]
fn test_type_alias_with_initspace() {
    type Scalar = f32;
    type Integer = i32;
    type Address = Pubkey;

    #[derive(InitSpace)]
    #[allow(dead_code)]
    struct TestStruct {
        x: Scalar,
        y: Integer,
        owner: Address,
    }

    // Should be 4 + 4 + 32 = 40
    assert_eq!(TestStruct::INIT_SPACE, 40);
}

// Test more complex scenarios with mixed primitive type aliases
#[test]
fn test_complex_type_aliases_with_initspace() {
    type Scalar = f32;
    type UserId = u64;
    type IsActive = bool;

    #[derive(InitSpace)]
    #[allow(dead_code)]
    struct ComplexStruct {
        x: Scalar,        // f32 = 4
        y: Scalar,        // f32 = 4
        z: Scalar,        // f32 = 4
        user_id: UserId,  // u64 = 8
        active: IsActive, // bool = 1
        #[max_len(10)]
        name: String, // 4 + 10 = 14
    }

    // Should be 4 + 4 + 4 + 8 + 1 + 14 = 35
    assert_eq!(ComplexStruct::INIT_SPACE, 35);
}

// Test that the fix works with arrays of primitive type aliases
#[test]
fn test_array_with_primitive_type_aliases() {
    type Pixel = u8;
    type Coordinate = i32;

    #[derive(InitSpace)]
    #[allow(dead_code)]
    struct ImageData {
        width: Coordinate,    // i32 = 4
        height: Coordinate,   // i32 = 4
        pixels: [Pixel; 100], // [u8; 100] = 100
    }

    // Should be 4 + 4 + 100 = 108
    assert_eq!(ImageData::INIT_SPACE, 108);
}

// Test the exact scenario from GitHub issue #3628
#[test]
fn test_github_issue_3628_scenario() {
    // This reproduces the exact issue mentioned in the GitHub issue
    pub type Scalar = f32;

    #[derive(InitSpace)]
    #[allow(dead_code)]
    pub struct Vector2 {
        pub x: Scalar,
        pub y: Scalar,
    }

    // Vector2 has two f32 fields, each taking 4 bytes
    // So INIT_SPACE should be 4 + 4 = 8
    assert_eq!(Vector2::INIT_SPACE, 8);

    // This should compile without any "trait bound f32: anchor_lang::Space is not satisfied" errors
    // which was the original issue
}
