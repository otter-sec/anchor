use anchor_lang::prelude::*;

// Regression test for #5303: a bare `Address` field — the spelling
// `use anchor_lang::prelude::*` brings into scope — must get a size bound,
// so `#[event]` emits `__anchor_with_event_cpi_data` and `emit_cpi!`
// compiles without a storage argument.
#[event]
pub struct AcceptAdmin {
    pub admin: Address,
}

#[test]
fn bare_address_event_emits_cpi_data_method() {
    let event = AcceptAdmin {
        admin: Address::default(),
    };
    let data = event
        .__anchor_with_event_cpi_data(|bytes| Ok(bytes.to_vec()))
        .expect("event CPI data method should exist and serialize");
    assert!(!data.is_empty());
    let plain = event
        .__anchor_with_event_data(|bytes| Ok(bytes.to_vec()))
        .expect("event data method should exist and serialize");
    assert!(!plain.is_empty());
}
