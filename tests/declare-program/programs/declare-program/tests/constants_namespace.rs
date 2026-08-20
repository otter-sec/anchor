use anchor_lang::prelude::*;

declare_program!(constants_namespace);
use constants_namespace::{constants::CONFIG, types::Mode};

#[test]
fn defined_constant_values_resolve_through_defined_namespace() {
    assert!(matches!(CONFIG.mode, Mode::Ready));
}
