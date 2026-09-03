//! Miri tests for `PodVec` — verify no UB under Tree Borrows for the
//! safe operations. The implementation relies on `unsafe impl Pod` and
//! `unsafe { mem::zeroed() }` in `Default::default`; this exercise those
//! paths under Miri to catch any Tree Borrows / provenance violation.
//!
//! Run: `cargo +nightly miri test --test miri_pod_vec`
//! (or: `MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test ...`)

use anchor_lang::pod::{PodU64, PodVec};

#[test]
fn default_is_empty() {
    let v: PodVec<PodU64, 8> = PodVec::default();
    assert_eq!(v.len(), 0);
    assert!(v.is_empty());
    assert_eq!(v.capacity(), 8);
}

#[test]
fn push_pop_roundtrip() {
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    for i in 0u64..4 {
        assert!(v.try_push(PodU64::from(i)).is_ok());
    }
    assert!(v.is_full());
    // Push to full must reject, not corrupt.
    assert!(v.try_push(PodU64::from(99)).is_err());
    // Pop LIFO.
    for i in (0u64..4).rev() {
        assert_eq!(v.pop().unwrap().get(), i);
    }
    assert!(v.is_empty());
    assert!(v.pop().is_none());
}

#[test]
fn indexing_and_iter() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    for i in 0u64..5 {
        v.push(PodU64::from(i * 10));
    }
    // Slice access matches index access.
    for i in 0..5 {
        assert_eq!(v[i].get(), (i as u64) * 10);
        assert_eq!(v.get(i).unwrap().get(), (i as u64) * 10);
    }
    // Iter covers populated elements only.
    let sum: u64 = v.iter().map(|p| p.get()).sum();
    assert_eq!(sum, 0 + 10 + 20 + 30 + 40);
}

#[test]
fn set_from_slice_and_read() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    let src: Vec<PodU64> = (0u64..6).map(PodU64::from).collect();
    v.set_from_slice(&src);
    assert_eq!(v.len(), 6);
    // Byte-level read back via the Pod trait — exercises the
    // `unsafe impl Pod` + alignment-1 invariant.
    let bytes: &[u8] = bytemuck::bytes_of(&v);
    // Layout is [len: u16 LE][data: PodU64; 8] = 2 + 8*8 = 66 bytes.
    assert_eq!(bytes.len(), 66);
    assert_eq!(bytes[0], 6); // len low byte
    assert_eq!(bytes[1], 0); // len high byte
}

#[test]
fn truncate_and_clear() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    for i in 0u64..5 {
        v.push(PodU64::from(i));
    }
    v.truncate(3);
    assert_eq!(v.len(), 3);
    v.clear();
    assert_eq!(v.len(), 0);
    assert!(v.is_empty());
}

#[test]
fn extend_then_pop_does_not_corrupt() {
    let mut v: PodVec<PodU64, 8> = PodVec::default();
    v.try_extend_from_slice(&[PodU64::from(1), PodU64::from(2), PodU64::from(3)])
        .unwrap();
    // Extend beyond capacity fails atomically — state unchanged.
    let big: Vec<PodU64> = (0u64..6).map(PodU64::from).collect();
    assert!(v.try_extend_from_slice(&big).is_err());
    assert_eq!(v.len(), 3);
    assert_eq!(v.pop().unwrap().get(), 3);
    assert_eq!(v.pop().unwrap().get(), 2);
    assert_eq!(v.pop().unwrap().get(), 1);
    assert!(v.pop().is_none());
}

#[test]
fn bytemuck_cast_roundtrip() {
    // Construct a PodVec by casting from raw bytes — the zero-copy path.
    let mut v: PodVec<PodU64, 4> = PodVec::default();
    v.push(PodU64::from(0xDEADBEEF));
    v.push(PodU64::from(0xCAFEBABE));

    let bytes: &[u8] = bytemuck::bytes_of(&v);
    let reloaded: &PodVec<PodU64, 4> = bytemuck::from_bytes(bytes);
    assert_eq!(reloaded.len(), 2);
    assert_eq!(reloaded[0].get(), 0xDEADBEEF);
    assert_eq!(reloaded[1].get(), 0xCAFEBABE);
}
