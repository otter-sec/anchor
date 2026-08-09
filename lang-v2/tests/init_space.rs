//! Compile-time tests for `#[derive(InitSpace)]`.
//!
//! Exercises every branch of `len_from_type` — primitives, arrays, tuples,
//! `Option<T>`, `String`, `Vec<T>` with `#[max_len]`, nested structs, and
//! enums. Each `INIT_SPACE` is a `const` so wrong answers fail at compile
//! time; runtime assertions double-check the values the derive generated.

// Test structs exist only to feed the derive — fields/variants aren't read.
#![allow(dead_code)]

use anchor_lang_v2::{prelude::*, InitSpace, Space};

#[derive(InitSpace)]
struct Primitives {
    _a: u8,
    _b: i8,
    _c: bool,
    _d: u16,
    _e: i16,
    _f: u32,
    _g: i32,
    _h: f32,
    _i: u64,
    _j: i64,
    _k: f64,
    _l: u128,
    _m: i128,
}
// u8(1) + i8(1) + bool(1) + u16(2) + i16(2) + u32(4) + i32(4) + f32(4)
// + u64(8) + i64(8) + f64(8) + u128(16) + i128(16) = 75
const PRIMITIVES_SPACE: usize = 75;

#[test]
fn primitives_sum_to_expected_byte_count() {
    assert_eq!(Primitives::INIT_SPACE, PRIMITIVES_SPACE);
}

#[derive(InitSpace)]
struct WithArray {
    _vals: [u64; 4],  // 4 * 8 = 32
    _bytes: [u8; 16], // 16 * 1 = 16
}

#[test]
fn array_length_multiplies_element_size() {
    assert_eq!(WithArray::INIT_SPACE, 32 + 16);
}

#[derive(InitSpace)]
struct WithAddress {
    _owner: Address, // 32
}

#[test]
fn address_counts_as_32_bytes() {
    assert_eq!(WithAddress::INIT_SPACE, 32);
}

#[derive(InitSpace)]
struct WithOption {
    _maybe: Option<u64>, // 1 + 8
}

#[test]
fn option_adds_one_byte_discriminator() {
    assert_eq!(WithOption::INIT_SPACE, 9);
}

#[derive(InitSpace)]
struct WithString {
    #[max_len(32)]
    _name: String, // 4 + 32
}

#[test]
fn string_reserves_max_len_plus_length_prefix() {
    assert_eq!(WithString::INIT_SPACE, 36);
}

#[derive(InitSpace)]
struct WithVec {
    #[max_len(10)]
    _xs: Vec<u64>, // 4 + 8 * 10
}

#[test]
fn vec_reserves_max_len_times_element_plus_prefix() {
    assert_eq!(WithVec::INIT_SPACE, 84);
}

#[derive(InitSpace)]
struct WithVecOfStrings {
    #[max_len(4, 16)]
    _xs: Vec<String>, // 4 + (4 + 16) * 4 = 84
}

#[test]
fn nested_max_len_args_flow_left_to_right() {
    assert_eq!(WithVecOfStrings::INIT_SPACE, 4 + (4 + 16) * 4);
}

#[derive(InitSpace)]
struct WithTuple {
    _pair: (u64, Address), // 8 + 32 = 40
}

#[test]
fn tuple_sums_element_sizes() {
    assert_eq!(WithTuple::INIT_SPACE, 40);
}

#[derive(InitSpace)]
struct Inner {
    _x: u64,
}

#[derive(InitSpace)]
struct Outer {
    _inner: Inner,    // uses Inner::INIT_SPACE = 8
    _arr: [Inner; 3], // 3 * 8 = 24
}

#[test]
fn nested_struct_uses_inner_init_space() {
    assert_eq!(Inner::INIT_SPACE, 8);
    assert_eq!(Outer::INIT_SPACE, 8 + 24);
}

#[derive(InitSpace)]
enum Variant {
    A,             // 0
    B(u8),         // 1
    C(u64, u64),   // 16
    D { _x: u32 }, // 4
}
// Enum size = 1 (disc) + max variant size = 1 + 16 = 17.

#[test]
fn enum_picks_largest_variant_plus_discriminator() {
    assert_eq!(Variant::INIT_SPACE, 1 + 16);
}

#[derive(InitSpace, wincode::SchemaRead, wincode::SchemaWrite)]
#[wincode(tag_encoding = "u32")]
enum WideTagVariant {
    A,
    B(u64),
    C { _x: u16 },
}

#[test]
fn enum_honors_wincode_tag_encoding_size() {
    assert_eq!(WideTagVariant::INIT_SPACE, 4 + 8);
}

#[derive(InitSpace)]
struct Unit;

#[test]
fn unit_struct_has_zero_init_space() {
    assert_eq!(Unit::INIT_SPACE, 0);
}

#[derive(InitSpace)]
struct TupleStruct(u64, u32);

#[test]
fn tuple_struct_sums_field_sizes() {
    assert_eq!(TupleStruct::INIT_SPACE, 8 + 4);
}
