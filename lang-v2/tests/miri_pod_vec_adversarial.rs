//! Adversarial Miri tests for `PodVec` — deliberately probing the
//! edges. Corrupted `len` field, boundary capacity, zero-capacity.
//!
//! Any test here that fails under Miri or produces a logic bug is a
//! real issue — disclose per `SECURITY.md`.

use anchor_lang::pod::{PodU64, PodVec};

// -- Boundary capacity -------------------------------------------------

#[test]
fn extend_lands_at_exactly_max() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    let src: Vec<PodU64> = (0u64..4).map(PodU64::from).collect();
    assert!(v.try_extend_from_slice(&src).is_ok());
    assert_eq!(v.len(), 4);
    assert!(v.is_full());
}

#[test]
fn extend_one_past_max_rejects_atomically() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    // Put 3 elements in.
    for i in 0u64..3 {
        v.push(PodU64::from(i));
    }
    // Try to extend by 2 — would land at 5, rejected.
    let src = vec![PodU64::from(10), PodU64::from(20)];
    assert!(v.try_extend_from_slice(&src).is_err());
    // State must be unchanged (the "atomic" claim).
    assert_eq!(v.len(), 3);
    for i in 0..3 {
        assert_eq!(v[i].get(), i as u64);
    }
}

#[test]
fn set_from_slice_at_exactly_max() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    let src: Vec<PodU64> = (0u64..4).map(PodU64::from).collect();
    v.set_from_slice(&src);
    assert_eq!(v.len(), 4);
}

#[test]
#[should_panic]
fn set_from_slice_one_past_max_panics() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    let src: Vec<PodU64> = (0u64..5).map(PodU64::from).collect();
    v.set_from_slice(&src);
}

// -- Empty state -------------------------------------------------------

#[test]
fn pop_on_empty_returns_none() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    assert!(v.pop().is_none());
    // Pop on empty must not affect state.
    assert_eq!(v.len(), 0);
    assert!(v.is_empty());
}

#[test]
fn clear_on_empty_is_idempotent() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    v.clear();
    assert_eq!(v.len(), 0);
    v.clear();
    assert_eq!(v.len(), 0);
}

#[test]
fn truncate_to_larger_is_noop() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    v.push(PodU64::from(1));
    v.push(PodU64::from(2));
    // truncate(5) with len=2 must be a no-op, not grow the len.
    v.truncate(5);
    assert_eq!(v.len(), 2);
}

// -- Corrupted length reads --------------------------------------------
//
// PodVec reads `len` from its u16 prefix. An attacker-controlled account
// buffer could have `len > MAX`. The `as_slice()` / `iter()` paths read
// `len` without clamping — this would produce a slice longer than `data`,
// UB for the read, or (worse) an OOB index on `data[..len]`.
//
// These tests detect that behavior. If `as_slice()` silently returns
// garbage, the assertion below won't catch it — but Miri WILL, because
// `&data[..len]` with len > MAX is an out-of-bounds slice.

#[test]
fn len_greater_than_max_bytemuck_path() {
    // Build a PodVec<PodU64, 4> via bytemuck cast from a buffer whose
    // `len` prefix claims 99. Layout: [len_lo=99][len_hi=0][data ... 32 bytes].
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99; // len = 99, but MAX = 4
                   // Reading .len() should return 99 — that's what the raw bytes say.
                   // Downstream slice access is where the bug lands.
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    assert_eq!(v.len(), 99);
    // `as_slice()` does `&self.data[..self.len()]` — with len=99 and
    // data.len()=4, this is an OOB slice. Must panic in debug,
    // catch in Miri.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _s = v.as_slice();
    }));
    assert!(
        result.is_err(),
        "as_slice() with corrupted len > MAX should panic — \
         if it silently returns a short slice, the PodVec API is unsound"
    );
}

#[test]
fn len_equals_max_plus_one_boundary() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 5; // len = 5, MAX = 4 — exactly one past
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _s = v.as_slice();
    }));
    assert!(result.is_err(), "len = MAX+1 must reject the slice read");
}

// -- Byte-level clobbering checks --------------------------------------

#[test]
fn bytes_after_len_match_initial_data() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    v.push(PodU64::from(0x1122334455667788));
    v.push(PodU64::from(0x99AABBCCDDEEFF00));

    let bytes = bytemuck::bytes_of(&v);
    // len prefix
    assert_eq!(bytes[0], 2);
    assert_eq!(bytes[1], 0);
    // First PodU64 in LE
    assert_eq!(&bytes[2..10], &0x1122334455667788u64.to_le_bytes()[..]);
    // Second
    assert_eq!(&bytes[10..18], &0x99AABBCCDDEEFF00u64.to_le_bytes()[..]);
    // Beyond len: unpopulated, but Default::default() zeroed them —
    // verify they're still zero (no write-past-len leaks).
    assert_eq!(&bytes[18..], &[0u8; 16][..]);
}

// -- pop does not read stale data as unsafe --------------------------

#[test]
fn pop_reads_last_populated_element() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    v.push(PodU64::from(111));
    v.push(PodU64::from(222));
    v.push(PodU64::from(333));
    // Pop returns the last-pushed value
    assert_eq!(v.pop().unwrap().get(), 333);
    assert_eq!(v.pop().unwrap().get(), 222);
    // After pop, data[1] and data[2] may still contain 222/333 raw —
    // that's an implementation detail, not an API leak. The exposed
    // length reflects what's been popped.
    assert_eq!(v.len(), 1);
    // Push a new element — it goes to position 1, overwriting 222's bytes.
    v.push(PodU64::from(444));
    assert_eq!(v[0].get(), 111);
    assert_eq!(v[1].get(), 444);
}
