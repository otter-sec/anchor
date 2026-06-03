use anchor_lang::prelude::*;

// Needed to declare accounts.
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

mod inside_mod {
    use super::*;

    #[derive(InitSpace)]
    pub struct Data {
        pub data: u64,
    }
}

#[derive(InitSpace)]
pub enum TestBasicEnum {
    Basic1,
    Basic2 {
        test_u8: u8,
    },
    Basic3 {
        test_u16: u16,
    },
    Basic4 {
        #[max_len(10)]
        test_vec: Vec<u8>,
    },
}

#[account]
#[derive(InitSpace)]
pub struct TestEmptyAccount {}

#[account]
#[derive(InitSpace)]
pub struct TestBasicVarAccount {
    pub test_u8: u8,
    pub test_u16: u16,
    pub test_u32: u32,
    pub test_u64: u64,
    pub test_u128: u128,
}

#[account]
#[derive(InitSpace)]
pub struct TestComplexVarAccount {
    pub test_key: Pubkey,
    #[max_len(10)]
    pub test_vec: Vec<u8>,
    #[max_len(10)]
    pub test_string: String,
    pub test_option: Option<u16>,
}

#[derive(InitSpace)]
pub struct TestNonAccountStruct {
    pub test_bool: bool,
}

#[account(zero_copy)]
#[derive(InitSpace)]
pub struct TestZeroCopyStruct {
    pub test_array: [u8; 8],
    pub test_u32: u32,
}

#[derive(InitSpace)]
pub struct ChildStruct {
    #[max_len(10)]
    pub test_string: String,
}

#[derive(InitSpace)]
pub struct TestNestedStruct {
    pub test_struct: ChildStruct,
    pub test_enum: TestBasicEnum,
}

#[derive(InitSpace)]
pub struct TestMatrixStruct {
    #[max_len(2, 4)]
    pub test_matrix: Vec<Vec<u8>>,
}

#[derive(InitSpace)]
pub struct TestFullPath {
    pub test_option_path: Option<inside_mod::Data>,
    pub test_path: inside_mod::Data,
}

const MAX_LEN: u8 = 10;

// Common size constants to reduce duplication
const PUBKEY_SIZE: usize = 32;
const VEC_LEN_SIZE: usize = 4;

#[derive(InitSpace)]
pub struct TestConst {
    #[max_len(MAX_LEN)]
    pub test_string: String,
    pub test_array: [u8; MAX_LEN as usize],
}

pub mod data {
    pub const SIZE: usize = 100;
}

#[derive(InitSpace)]
pub struct TestModuleConst {
    #[max_len(data::SIZE)]
    pub test_string: String,
    pub test_array: [u8; data::SIZE],
}

#[derive(InitSpace)]
pub struct TestUnnamedStruct(
    pub u8,
    #[max_len(4)] pub Vec<u32>,
    #[max_len(10)] pub String,
    pub ChildStruct,
    pub TestBasicEnum,
);

#[derive(InitSpace)]
pub struct TestUnitStruct;

#[derive(InitSpace)]
#[allow(clippy::type_complexity)]
pub struct TestTupleStruct {
    pub test_tuple: (u8, u16, u32, u64, u128),
    pub mixed_tuple: (bool, f32, f64, i8, i16, i32, i64, i128),

    pub nested_tuple: (u8, (u16, u32, u64, u128)),
    pub deeply_nested: (u8, (u16, (u32, (u64, u128)))),
    pub complex_nested: (bool, (u8, u16), (u32, (u64, u128))),

    pub option_tuple: Option<(u8, u16, u32, u64, u128)>,
    pub tuple_with_option: (u8, Option<u16>, u32),
    pub nested_option_tuple: (u8, Option<(u16, u32)>, u64),

    pub pubkey_tuple: (Pubkey, u64),
    pub tuple_with_pubkeys: (Pubkey, Pubkey, u8),

    pub struct_tuple: (ChildStruct, u8),
    pub nested_struct_tuple: (u8, (ChildStruct, u16)),

    pub single_tuple: (u64,),
    pub single_nested: ((u8,),),

    pub empty_tuple: (),
    pub tuple_with_empty: (u8, (), u16),

    pub array_tuple: ([u8; 4], u16),
    pub tuple_array_nested: (u8, ([u16; 2], u32)),

    pub ultimate_complex: (u8, (bool, Option<(u16, u32)>, ChildStruct), Pubkey),
}

#[test]
fn test_empty_struct() {
    assert_eq!(TestEmptyAccount::INIT_SPACE, 0);
}

#[test]
fn test_basic_struct() {
    assert_eq!(TestBasicVarAccount::INIT_SPACE, 1 + 2 + 4 + 8 + 16);
}

#[test]
fn test_complex_struct() {
    assert_eq!(
        TestComplexVarAccount::INIT_SPACE,
        32 + 4 + 10 + (4 + 10) + 3
    )
}

#[test]
fn test_zero_copy_struct() {
    assert_eq!(TestZeroCopyStruct::INIT_SPACE, 8 + 4)
}

#[test]
fn test_basic_enum() {
    assert_eq!(TestBasicEnum::INIT_SPACE, 1 + 14);
}

#[test]
fn test_nested_struct() {
    assert_eq!(
        TestNestedStruct::INIT_SPACE,
        ChildStruct::INIT_SPACE + TestBasicEnum::INIT_SPACE
    )
}

#[test]
fn test_matrix_struct() {
    assert_eq!(TestMatrixStruct::INIT_SPACE, 4 + (2 * (4 + 4)))
}

#[test]
fn test_full_path() {
    assert_eq!(TestFullPath::INIT_SPACE, 8 + 9)
}

#[test]
fn test_const() {
    assert_eq!(TestConst::INIT_SPACE, (4 + 10) + 10)
}

#[derive(InitSpace)]
struct OptionVecStruct {
    #[max_len(8)]
    pub maybe_data: Option<Vec<u8>>,
}

#[derive(InitSpace)]
struct DeepNestedVec {
    #[max_len(2, 3, 4)]
    pub data: Vec<Vec<Vec<u8>>>,
}

#[test]
fn test_option_vec_struct() {
    assert_eq!(OptionVecStruct::INIT_SPACE, 1 + 4 + 8);
    let option_vec = OptionVecStruct {
        maybe_data: Some(vec![1, 2, 3, 4]),
    };

    // Use the field to avoid warning
    assert_eq!(option_vec.maybe_data, Some(vec![1, 2, 3, 4]));
}

#[test]
fn test_deeply_nested_vec() {
    assert_eq!(DeepNestedVec::INIT_SPACE, 4 + (2 * (4 + (3 * (4 + 4)))));
    let deep_nested = DeepNestedVec {
        data: vec![vec![vec![1, 2, 3, 4]]],
    };

    // Use the field to avoid warning
    assert_eq!(deep_nested.data, vec![vec![vec![1, 2, 3, 4]]]);
}

#[test]
fn test_empty_enum() {
    #[derive(InitSpace)]
    enum EmptyEnum {}
    assert_eq!(EmptyEnum::INIT_SPACE, 1);
}

#[test]
fn test_module_const() {
    assert_eq!(
        TestModuleConst::INIT_SPACE,
        (VEC_LEN_SIZE + data::SIZE) + data::SIZE
    )
}

#[test]
fn test_module_const_comprehensive() {
    // Test the exact scenario from the GitHub issue using the top-level data module
    #[derive(InitSpace)]
    pub struct SomeData {
        pub owner: Pubkey,
        #[max_len(data::SIZE)]
        pub data: Vec<u8>,
    }

    // The space should be: 32 (Pubkey) + 4 (Vec length) + 100 (max data size) = 136
    assert_eq!(
        SomeData::INIT_SPACE,
        PUBKEY_SIZE + VEC_LEN_SIZE + data::SIZE
    );

    // Test that we can create an instance and use the fields
    let test_pubkey = Pubkey::new_unique();
    let test_data = vec![1, 2, 3, 4, 5];
    let some_data = SomeData {
        owner: test_pubkey,
        data: test_data.clone(),
    };

    // Verify the fields work correctly
    assert_eq!(some_data.owner, test_pubkey);
    assert_eq!(some_data.data, test_data);

    // Test that simple constants still work
    const SIZE: usize = 50;

    #[derive(InitSpace)]
    pub struct SomeDataSimple {
        pub owner: Pubkey,
        #[max_len(SIZE)]
        pub data: Vec<u8>,
    }

    assert_eq!(
        SomeDataSimple::INIT_SPACE,
        PUBKEY_SIZE + VEC_LEN_SIZE + SIZE
    );

    // Test that const assignments work
    const SIZE_ASSIGNED: usize = data::SIZE;

    #[derive(InitSpace)]
    pub struct SomeDataAssigned {
        pub owner: Pubkey,
        #[max_len(SIZE_ASSIGNED)]
        pub data: Vec<u8>,
    }

    assert_eq!(
        SomeDataAssigned::INIT_SPACE,
        PUBKEY_SIZE + VEC_LEN_SIZE + data::SIZE
    );

    // Test that we can create instances of these structs too
    let simple_data = SomeDataSimple {
        owner: test_pubkey,
        data: test_data.clone(),
    };

    let assigned_data = SomeDataAssigned {
        owner: test_pubkey,
        data: test_data,
    };

    // Verify all instances work correctly
    assert_eq!(simple_data.owner, test_pubkey);
    assert_eq!(assigned_data.owner, test_pubkey);
    assert_eq!(simple_data.data, vec![1, 2, 3, 4, 5]);
    assert_eq!(assigned_data.data, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_unnamed_struct() {
    assert_eq!(
        TestUnnamedStruct::INIT_SPACE,
        1 + 4 + 4 * 4 + 4 + 10 + ChildStruct::INIT_SPACE + TestBasicEnum::INIT_SPACE
    )
}

#[test]
fn test_unit_struct() {
    assert_eq!(TestUnitStruct::INIT_SPACE, 0)
}

#[test]
fn test_basic_tuple() {
    let basic_tuple_size = 1 + 2 + 4 + 8 + 16; // 31
    assert!(TestTupleStruct::INIT_SPACE >= basic_tuple_size);
}

#[test]
fn test_tuple_space_calculations() {
    let basic_tuple_size = 1 + 2 + 4 + 8 + 16; // 31

    let mixed_tuple_size = 1 + 4 + 8 + 1 + 2 + 4 + 8 + 16; // 44

    let nested_tuple_size = 1 + (2 + 4 + 8 + 16); // 31

    let option_tuple_size = 1 + (1 + 2 + 4 + 8 + 16); // 32

    let pubkey_tuple_size = 32 + 8; // 40

    let single_tuple_size = 8;

    let empty_tuple_size = 0;

    let minimum_expected_size = basic_tuple_size
        + mixed_tuple_size
        + nested_tuple_size
        + option_tuple_size
        + pubkey_tuple_size
        + single_tuple_size
        + empty_tuple_size;

    assert!(TestTupleStruct::INIT_SPACE >= minimum_expected_size);
}

#[test]
fn test_tuple_with_structs() {
    // Test that tuples containing other structs work correctly
    // struct_tuple: (ChildStruct, u8) = ChildStruct::INIT_SPACE + 1
    let expected_struct_tuple_contribution = ChildStruct::INIT_SPACE + 1;

    assert!(TestTupleStruct::INIT_SPACE >= expected_struct_tuple_contribution);
}

#[test]
fn test_nested_tuple_complexity() {
    // Test deeply_nested: (u8, (u16, (u32, (u64, u128))))
    // = 1 + (2 + (4 + (8 + 16))) = 1 + (2 + (4 + 24)) = 1 + (2 + 28) = 1 + 30 = 31
    let deeply_nested_size = 1 + 2 + 4 + 8 + 16; // 31

    // Test complex_nested: (bool, (u8, u16), (u32, (u64, u128)))
    // = 1 + (1 + 2) + (4 + (8 + 16)) = 1 + 3 + (4 + 24) = 1 + 3 + 28 = 32
    let complex_nested_size = 1 + (1 + 2) + (4 + (8 + 16)); // 32

    assert!(TestTupleStruct::INIT_SPACE >= deeply_nested_size + complex_nested_size);
}

#[test]
fn test_tuple_with_options() {
    // tuple_with_option: (u8, Option<u16>, u32) = 1 + (1 + 2) + 4 = 8
    let tuple_with_option_size = 1 + (1 + 2) + 4; // 8

    // nested_option_tuple: (u8, Option<(u16, u32)>, u64) = 1 + (1 + (2 + 4)) + 8 = 16
    let nested_option_tuple_size = 1 + (1 + (2 + 4)) + 8; // 16

    assert!(TestTupleStruct::INIT_SPACE >= tuple_with_option_size + nested_option_tuple_size);
}

#[test]
fn test_tuple_with_arrays() {
    // array_tuple: ([u8; 4], u16) = (4 * 1) + 2 = 6
    let array_tuple_size = 4 + 2; // 6

    // tuple_array_nested: (u8, ([u16; 2], u32)) = 1 + ((2 * 2) + 4) = 1 + (4 + 4) = 9
    let tuple_array_nested_size = 1 + ((2 * 2) + 4); // 9

    assert!(TestTupleStruct::INIT_SPACE >= array_tuple_size + tuple_array_nested_size);
}
