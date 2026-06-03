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
