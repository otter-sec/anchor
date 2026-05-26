use {anchor_lang_v2::Id, declare_program_constants::constants, solana_pubkey::Pubkey};

fn constants_id() -> Pubkey {
    "11111111111111111111111111111111".parse().unwrap()
}

#[test]
fn declared_constants_exports_program_marker_and_id() {
    assert_eq!(constants::ID, constants_id());
    assert_eq!(constants::program::Constants::id(), constants_id());
}

#[test]
fn declared_constants_preserve_v1_constant_shapes() {
    assert_eq!(constants::constants::DOC_BYTES, b"doc");
    assert_eq!(constants::constants::TEXT, "constant text");
    assert_eq!(constants::constants::PROGRAM_KEY, Pubkey::default());
    assert_eq!(constants::constants::QUOTED_PROGRAM_KEY, Pubkey::default());
    assert_eq!(constants::constants::FIXED_BYTES, [1, 2, 3, 4]);
    assert_eq!(constants::constants::FIXED_WORDS, [10, 20, 30]);
    assert!(constants::constants::ENABLED);
    assert_eq!(constants::constants::SIGNED, -9);
    assert_eq!(constants::constants::WIDE, u128::MAX);
    assert_eq!(constants::constants::RATIO, 1.25);

    fn assert_static_bytes(_: &'static [u8]) {}
    fn assert_static_str(_: &'static str) {}

    assert_static_bytes(constants::constants::DOC_BYTES);
    assert_static_str(constants::constants::TEXT);
}
