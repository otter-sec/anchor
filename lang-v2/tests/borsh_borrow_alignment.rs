use anchor_lang::{wincode, BORSH_CONFIG};

#[test]
fn borsh_config_rejects_misaligned_borrowed_wide_slices() {
    let values = [11_u64, 22, 33];
    let encoded = wincode::config::serialize(&values[..], BORSH_CONFIG).unwrap();

    let err = wincode::config::deserialize::<&[u64], _>(&encoded, BORSH_CONFIG).unwrap_err();
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
