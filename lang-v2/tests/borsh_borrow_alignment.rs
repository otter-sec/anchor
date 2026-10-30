use anchor_lang::{wincode, BORSH_CONFIG};

#[repr(align(8))]
struct AlignedBytes<const N: usize>([u8; N]);

#[test]
fn borsh_config_rejects_misaligned_borrowed_wide_slices() {
    let values = [11_u64, 22, 33];
    let encoded = wincode::config::serialize(&values[..], BORSH_CONFIG).unwrap();

    // A Vec<u8> does not guarantee a particular base alignment, so relying on
    // its allocator layout makes this test pass or fail depending on the host.
    // The one-byte offset from an explicitly eight-byte-aligned buffer makes
    // the borrowed u64 payload unaligned on every supported architecture.
    let mut storage = AlignedBytes::<64>([0; 64]);
    storage.0[1..1 + encoded.len()].copy_from_slice(&encoded);
    let misaligned = &storage.0[1..1 + encoded.len()];

    let err =
        wincode::config::deserialize::<&[u64], _>(misaligned, BORSH_CONFIG).unwrap_err();
    assert!(
        err.to_string().contains("unaligned"),
        "expected an unaligned-reference error, got: {err}"
    );
}

#[test]
fn borsh_config_still_deserializes_owned_wide_vectors() {
    let values = vec![11_u64, 22, 33];
    let encoded = wincode::config::serialize(&values, BORSH_CONFIG).unwrap();

    let decoded: Vec<u64> = wincode::config::deserialize(&encoded, BORSH_CONFIG).unwrap();
    assert_eq!(decoded, values);
}
