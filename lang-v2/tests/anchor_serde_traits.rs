//! Wincode accepts NaN on Anchor v2 Borsh-shaped surfaces.

use anchor_lang::{AnchorDeserialize, AnchorSerialize, BORSH_CONFIG};

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct FloatPayload {
    pub value: f64,
}

#[test]
fn wincode_accepts_nan_on_borsh_shaped_surfaces() {
    let value = FloatPayload { value: f64::NAN };
    let bytes = anchor_lang::wincode::config::serialize(&value, BORSH_CONFIG).unwrap();
    let decoded: FloatPayload =
        anchor_lang::wincode::config::deserialize(&bytes, BORSH_CONFIG).unwrap();

    assert!(decoded.value.is_nan());
}
