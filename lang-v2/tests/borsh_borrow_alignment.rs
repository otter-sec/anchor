use anchor_lang::{wincode, FixedVec, BORSH_CONFIG};

#[test]
fn borsh_config_rejects_misaligned_borrowed_wide_slices() {
    let values = [11_u64, 22, 33];
    let encoded = wincode::config::serialize(&values[..], BORSH_CONFIG).unwrap();

    // The four-byte length prefix must leave the u64 payload misaligned.
    #[repr(align(8))]
    struct AlignedBytes([u8; 28]);
    let mut buffer = AlignedBytes([0; 28]);
    buffer.0.copy_from_slice(&encoded);

    let err = wincode::config::deserialize::<&[u64], _>(&buffer.0, BORSH_CONFIG).unwrap_err();
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

#[test]
fn fixed_vectors_decode_wide_and_nested_borsh_values_without_alignment_casts() {
    let mut inner = FixedVec::<u64, 3>::new();
    inner.try_push(11).unwrap();
    inner.try_push(22).unwrap();
    inner.try_push(33).unwrap();

    let mut values = FixedVec::<FixedVec<u64, 3>, 2>::new();
    values.try_push(inner).unwrap();

    let encoded = wincode::config::serialize(&values, BORSH_CONFIG).unwrap();
    let decoded: FixedVec<FixedVec<u64, 3>, 2> =
        wincode::config::deserialize(&encoded, BORSH_CONFIG).unwrap();

    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].as_slice(), &[11, 22, 33]);
}
