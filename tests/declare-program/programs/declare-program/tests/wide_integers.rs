use anchor_lang::{prelude::*, InstructionData};

declare_program!(wide_integers);

#[test]
fn u256_and_i256_are_32_little_endian_bytes() {
    let mut signed = [0xff; 32];
    signed[0] = 0xfe; // -2
    let wide = wide_integers::types::Wide {
        unsigned: [7; 32],
        signed,
    };
    let data = wide_integers::client::args::StoreWide { wide }.data();
    assert_eq!(
        data,
        [&[1, 2, 3, 4, 5, 6, 7, 8][..], &[7; 32], &signed].concat()
    );
}
