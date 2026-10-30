//! Tests for the `PodVec` validation API — `is_valid_len`, `validate`,
//! `try_as_slice`, `try_as_mut_slice`, `try_pop`, `try_get`.
//!
//! Background: `PodVec<T, MAX>` exposes a raw `len()` that reflects the
//! in-memory u16 length prefix without clamping to `MAX`. When the
//! source buffer comes from attacker-controlled account bytes, the
//! prefix may exceed `MAX` — the unchecked accessors (`as_slice`,
//! `pop`, `iter`, `get`, indexing) panic via Rust's bounds check in
//! that state. Programs that want to detect the corruption up-front
//! (instead of relying on a transaction-aborting panic) use the
//! `try_*` variants exercised here.
//!
//! Paired with the compile-time `MAX > u16::MAX` guard (`PodVec::CAPACITY` /
//! `Default` / `#[account]` field diagnostics), this closes the remaining
//! runtime-validation gap documented in the PodVec API comments.

use anchor_lang::pod::{CapacityError, PodU64, PodVec};

// -- is_valid_len / validate ---------------------------------------------

#[test]
fn is_valid_len_true_on_fresh_default() {
    let v: PodVec<PodU64, 4> = PodVec::default();
    assert!(v.is_valid_len());
    assert_eq!(v.validate(), Ok(()));
}

#[test]
fn is_valid_len_true_when_at_max() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    for i in 0u64..4 {
        v.push(PodU64::from(i));
    }
    assert!(v.is_valid_len());
}

#[test]
fn is_valid_len_false_when_prefix_exceeds_max() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99; // len = 99, MAX = 4
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    assert!(!v.is_valid_len());
    assert_eq!(v.validate(), Err(CapacityError));
}

#[test]
fn is_valid_len_edge_max_plus_one() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 5; // len = 5, MAX = 4 — exactly one past
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    assert!(!v.is_valid_len());
}

// -- try_as_slice --------------------------------------------------------

#[test]
fn try_as_slice_returns_slice_when_valid() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    v.push(PodU64::from(10));
    v.push(PodU64::from(20));
    v.push(PodU64::from(30));

    let slice = v.try_as_slice().expect("valid len must produce Ok");
    assert_eq!(slice.len(), 3);
    assert_eq!(slice[0].get(), 10);
    assert_eq!(slice[2].get(), 30);
}

#[test]
fn try_as_slice_errors_on_corrupted_len() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);

    // Key guarantee: returns Err, does NOT panic.
    assert_eq!(v.try_as_slice().err(), Some(CapacityError));
}

#[test]
fn try_as_mut_slice_errors_on_corrupted_len() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &mut PodVec<PodU64, 4> = bytemuck::from_bytes_mut(&mut bytes);

    assert_eq!(v.try_as_mut_slice().err(), Some(CapacityError));
}

// -- try_pop --------------------------------------------------------------

#[test]
fn try_pop_returns_none_on_empty_valid_vec() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    assert_eq!(v.try_pop(), Ok(None));
}

#[test]
fn try_pop_returns_last_element_on_valid_vec() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    v.push(PodU64::from(7));
    v.push(PodU64::from(42));

    let popped = v.try_pop().expect("valid len must produce Ok");
    assert_eq!(popped.unwrap().get(), 42);
    assert_eq!(v.len(), 1);
}

#[test]
fn try_pop_errors_on_corrupted_len() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &mut PodVec<PodU64, 4> = bytemuck::from_bytes_mut(&mut bytes);

    // Key guarantee: unchecked `pop()` would panic at `self.data[98]`
    // on a `[T; 4]`. `try_pop` returns Err cleanly.
    assert_eq!(v.try_pop().err(), Some(CapacityError));

    // State is unchanged — try_pop must not mutate on error.
    assert_eq!(v.len(), 99);
}

// -- try_get --------------------------------------------------------------

#[test]
fn try_get_returns_element_when_in_bounds() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    for i in 0u64..3 {
        v.push(PodU64::from(i * 100));
    }

    assert_eq!(v.try_get(0).unwrap().unwrap().get(), 0);
    assert_eq!(v.try_get(2).unwrap().unwrap().get(), 200);
}

#[test]
fn try_get_returns_ok_none_when_idx_out_of_populated_range() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    v.push(PodU64::from(1));

    // len == 1, idx 5 is out of bounds but the *length prefix* is valid.
    assert_eq!(v.try_get(5), Ok(None));
}

#[test]
fn try_get_errors_on_corrupted_len() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);

    assert_eq!(v.try_get(0).err(), Some(CapacityError));
}

// -- validate-then-use pattern (defensive coding style) -----------------

#[test]
fn validate_once_then_unchecked_access_is_safe() {
    // Mirrors the intended consumer pattern: validate at account load,
    // then trust the buffer for subsequent access.
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    for i in 0u64..4 {
        v.push(PodU64::from(i));
    }
    let bytes: Vec<u8> = bytemuck::bytes_of(&v).to_vec();
    let reloaded: &PodVec<PodU64, 8> = bytemuck::from_bytes(&bytes);

    // Account load: validate once.
    reloaded.validate().expect("buffer valid");

    // Subsequent unchecked access is safe.
    assert_eq!(reloaded.as_slice().len(), 4);
    assert_eq!(reloaded[2].get(), 2);
    let sum: u64 = reloaded.iter().map(|p| p.get()).sum();
    assert_eq!(sum, 0 + 1 + 2 + 3);
}
