//! Does `PodVec` behave safely when its in-memory `len` field has
//! been corrupted to a value > MAX? An attacker-controlled Solana
//! account can produce this shape.
//!
//! Existing behavior observation:
//! - `len()` returns the corrupted value verbatim (e.g., 99 for
//!   MAX=4). No clamp.
//! - `as_slice()` / `iter()` panic on `&data[..len]` OOB.
//! - `try_push` checks `if len >= MAX` and rejects — safe.
//! - `pop` computes `data[len - 1]` without a MAX guard. With
//!   `len=99` and MAX=4, this indexes `data[98]` which is OOB —
//!   panics at Rust's bounds check. Not UB, but it's a DoS vector if
//!   a program calls `pop` on an attacker-supplied PodVec without
//!   first validating `len() <= MAX`.
//!
//! This test documents the current behavior. If it starts failing,
//! the semantics shifted and the v2 docs / API guidance should be
//! reviewed.

use anchor_lang::pod::{PodU64, PodVec};

#[test]
fn len_is_not_clamped_to_max_on_read() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    // `.len()` exposes the raw value. Callers must validate before trust.
    assert_eq!(v.len(), 99);
}

#[test]
fn try_push_rejects_when_len_already_past_max() {
    // try_push guards with `if len >= MAX` — safe under corrupted len.
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &mut PodVec<PodU64, 4> = bytemuck::from_bytes_mut(&mut bytes);
    assert!(v.try_push(PodU64::from(42)).is_err());
    // State untouched.
    assert_eq!(v.len(), 99);
}

#[test]
#[should_panic]
fn pop_with_corrupted_len_panics() {
    // pop computes `self.data[len - 1]` without a MAX check. With
    // len > MAX it's an OOB index. Panic (not UB) is acceptable — it
    // aborts the transaction. But callers should validate first.
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &mut PodVec<PodU64, 4> = bytemuck::from_bytes_mut(&mut bytes);
    let _ = v.pop();
}

#[test]
#[should_panic]
fn as_slice_with_corrupted_len_panics() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    let _ = v.as_slice();
}

#[test]
#[should_panic]
fn index_with_corrupted_len_panics() {
    let mut bytes = [0u8; 2 + 8 * 4];
    bytes[0] = 99;
    let v: &PodVec<PodU64, 4> = bytemuck::from_bytes(&bytes);
    let _ = v[0];
}
