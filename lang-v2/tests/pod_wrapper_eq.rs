//! Tests for the validating `PartialEq` impls emitted by `#[pod_wrapper]`.
//!
//! Background: the generated `PodEnum` wraps a raw `u8`, so any byte pattern
//! is representable. Earlier revisions of the macro compared the raw bytes
//! directly in `PartialEq`, which let an invalid discriminant silently
//! participate in `==` / `!=`. A negative guard like
//! `if engine.market_mode != MarketMode::Closed { … sensitive … }` would then
//! evaluate `42 != 2` as `true` and execute the sensitive branch with a
//! corrupt state byte.
//!
//! The current macro validates both operands against the declared variants
//! (via the existing `From<PodEnum> for Enum` match) and panics on an
//! unknown byte — mirroring the panic `.into()` would produce. Raw-byte
//! inspection via the public `PodEnum::0` field remains the unvalidated
//! escape hatch.
//!
//! These tests cover:
//!   * both sides of each cross-type impl panic on an invalid byte,
//!   * declared variants still compare correctly (positive and negative),
//!   * same-type pod-pod comparison also validates (no silent bypass).

#![allow(dead_code)]

use anchor_lang::prelude::*;

#[pod_wrapper]
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum MarketMode {
    Live = 0,
    Resolved = 1,
}

// -- positive cases -------------------------------------------------------

#[test]
fn pod_eq_enum_valid_same_variant() {
    assert!(PodMarketMode::Live == MarketMode::Live);
}

#[test]
fn enum_eq_pod_valid_same_variant() {
    assert!(MarketMode::Live == PodMarketMode::Live);
}

#[test]
fn pod_ne_enum_valid_different_variant() {
    assert!(PodMarketMode::Live != MarketMode::Resolved);
}

#[test]
fn pod_eq_pod_valid_same_variant() {
    assert!(PodMarketMode::Live == PodMarketMode::Live);
}

#[test]
fn pod_ne_pod_valid_different_variant() {
    assert!(PodMarketMode::Live != PodMarketMode::Resolved);
}

// -- invalid-byte cases — every comparison path must panic ---------------

#[test]
#[should_panic(expected = "invalid MarketMode discriminant: 42")]
fn pod_eq_pod_panics_when_left_invalid() {
    // Raw-byte construction simulates attacker-controlled / zero-init bytes.
    let bad = PodMarketMode(42);
    // `==` on an invalid operand must panic, not silently compare bytes.
    let _ = bad == PodMarketMode::Live;
}

#[test]
#[should_panic(expected = "invalid MarketMode discriminant: 42")]
fn pod_ne_enum_panics_on_invalid_byte() {
    // Regression for the negative-guard bypass: `invalid != Live` used to
    // evaluate `true` and fall through. It must panic now.
    let bad = PodMarketMode(42);
    let _ = bad != MarketMode::Live;
}

#[test]
#[should_panic(expected = "invalid MarketMode discriminant: 42")]
fn enum_eq_pod_panics_on_invalid_byte() {
    // Symmetric cross-type impl — `Enum == PodEnum(invalid)` also panics.
    let bad = PodMarketMode(42);
    let _ = MarketMode::Live == bad;
}

#[test]
#[should_panic(expected = "invalid MarketMode discriminant: 7")]
fn pod_eq_pod_panics_when_right_invalid() {
    // Pod-pod path validates both operands; invalid on the right still panics.
    let bad = PodMarketMode(7);
    let _ = PodMarketMode::Live == bad;
}

// -- regression: discriminant 0 is valid when a variant claims it --------

#[test]
fn pod_eq_pod_zero_byte_is_valid_variant() {
    // `MarketMode::Live = 0`, so `PodMarketMode(0)` round-trips through
    // validation. Zero-initialised Pod bytes must not spuriously panic when
    // they correspond to a real variant.
    let a = PodMarketMode(0);
    let b = PodMarketMode(0);
    assert!(a == b);
}
