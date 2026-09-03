//! Discriminator **formula** spec check.
//!
//!   account disc    = sha256("account:{name}")[..8]
//!   instruction disc = sha256("global:{fn_name}")[..8]
//!
//! Scope note: this test does NOT exercise the `#[account]` proc-macro
//! emission path. It asserts the sha256-vs-byte-literal relationship
//! using hand-written `impl Discriminator` blocks that mirror what the
//! derive emits. The value is catching drift in the *recipe* (e.g., if
//! someone accidentally changed the prefix to `"acct:"` or the length
//! to 10 bytes in the spec while the derive stayed put, or vice
//! versa). Real proc-macro emission is exercised implicitly by the
//! end-to-end programs under `tests/` that use `#[account]` and boot
//! against litesvm.

use anchor_lang::Discriminator;
use bytemuck::{Pod, Zeroable};
use sha2::{Digest, Sha256};

fn expected_account_disc(name: &str) -> [u8; 8] {
    let h = Sha256::digest(format!("account:{name}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&h[..8]);
    out
}

// Minimal #[account]-compatible type: Pod + Zeroable + repr(C) + Copy.
// Using `#[account]` directly would require owner/space/hash machinery;
// for the disc check, implementing Discriminator by hand via the same
// hashing approach as the macro produces the same byte-level contract.

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Counter {
    _count: u64,
}

// This impl matches what `#[account]` would emit verbatim (compile-time
// sha256 is baked to a byte literal in the real derive).
impl Discriminator for Counter {
    const DISCRIMINATOR: &'static [u8] = &[
        // sha256("account:Counter")[..8]
        //   python3 -c "import hashlib; print(hashlib.sha256(b'account:Counter').hexdigest()[:16])"
        // → ffb004f5bcfd7c19
        0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19,
    ];
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vault {
    _balance: u64,
}

impl Discriminator for Vault {
    // sha256("account:Vault")[..8] = d308e82b02987577
    const DISCRIMINATOR: &'static [u8] = &[0xd3, 0x08, 0xe8, 0x2b, 0x02, 0x98, 0x75, 0x77];
}

#[test]
fn counter_disc_matches_sha256_spec() {
    let expected = expected_account_disc("Counter");
    assert_eq!(Counter::DISCRIMINATOR, &expected[..]);
}

#[test]
fn vault_disc_matches_sha256_spec() {
    let expected = expected_account_disc("Vault");
    assert_eq!(Vault::DISCRIMINATOR, &expected[..]);
}

#[test]
fn disc_length_is_eight() {
    assert_eq!(Counter::DISCRIMINATOR.len(), 8);
    assert_eq!(Vault::DISCRIMINATOR.len(), 8);
}

#[test]
fn disc_is_prefix_of_sha256() {
    let full = Sha256::digest(b"account:Counter");
    assert_eq!(&full[..8], Counter::DISCRIMINATOR);
}
