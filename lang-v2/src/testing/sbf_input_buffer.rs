//! Mock serialized-input buffer matching the SBF loader's layout.
//!
//! Used by the cursor-walk Miri tests to exercise `AccountCursor::next`
//! on a realistic `num_accounts + per-account records` layout without
//! booting the SVM.
//!
//! Layout:
//!   num_accounts: u64 (8 bytes, LE)
//!   per-account record:
//!     if non-dup:
//!       RuntimeAccount header (88 bytes, borrow_state = NON_DUP_MARKER = 255)
//!       account data (data_len bytes)
//!       padding (MAX_PERMITTED_DATA_INCREASE = 10,240 bytes)
//!       rent_epoch (8 bytes)
//!       alignment padding to 8-byte boundary
//!     if dup:
//!       dup_marker (1 byte, value = index of earlier account)
//!       padding (7 bytes) to round to 8-byte alignment

use {
    alloc::{vec, vec::Vec},
    pinocchio::account::MAX_PERMITTED_DATA_INCREASE,
};

/// Serialized-input buffer simulating what the SBF loader writes.
///
/// **Alignment matters.** The real SBF loader aligns the input buffer
/// to 8 bytes (u64 alignment) because `RuntimeAccount` requires it
/// (`cursor.rs:136` does `*account.borrow_state` through
/// `*mut RuntimeAccount`, and Rust's `*mut T` dereference requires
/// `T`'s alignment).
///
/// `Vec<u8>` gives only u8 alignment. So we back with `Vec<u64>` and
/// expose the bytes via raw pointer cast. `Vec<u64>` guarantees at
/// least 8-byte alignment from the allocator.
pub struct SbfInputBuffer {
    // Backing store — 8-byte-aligned because element type is u64.
    backing: Vec<u64>,
    // Logical byte length (may be less than backing.len() * 8).
    len: usize,
    /// Byte offset where each account record starts. Useful for
    /// cursor-walk tests that need to reason about positions.
    pub record_offsets: Vec<usize>,
}

#[derive(Clone, Copy)]
pub enum AccountRecord {
    /// Non-duplicate account with the given header + data.
    NonDup {
        address: [u8; 32],
        owner: [u8; 32],
        lamports: u64,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
        data_len: usize,
    },
    /// Duplicate of an earlier account at `index`.
    Dup { index: u8 },
}

impl SbfInputBuffer {
    /// Build a serialized input buffer from a list of account records.
    /// Non-dup records zero-fill their data region (matching SVM behavior
    /// for fresh accounts).
    pub fn build(records: &[AccountRecord]) -> Self {
        // Compute total byte length first; collect bytes into a temp
        // Vec<u8>, then move into a 8-aligned Vec<u64> backing.
        let mut bytes: Vec<u8> = Vec::new();
        let mut record_offsets = Vec::with_capacity(records.len());

        bytes.extend_from_slice(&(records.len() as u64).to_le_bytes());

        for record in records {
            while bytes.len() % 8 != 0 {
                bytes.push(0);
            }
            record_offsets.push(bytes.len());

            match *record {
                AccountRecord::NonDup {
                    address,
                    owner,
                    lamports,
                    is_signer,
                    is_writable,
                    executable,
                    data_len,
                } => {
                    bytes.push(pinocchio::account::NOT_BORROWED);
                    bytes.push(is_signer as u8);
                    bytes.push(is_writable as u8);
                    bytes.push(executable as u8);
                    bytes.extend_from_slice(&[0u8; 4]);
                    bytes.extend_from_slice(&address);
                    bytes.extend_from_slice(&owner);
                    bytes.extend_from_slice(&lamports.to_le_bytes());
                    bytes.extend_from_slice(&(data_len as u64).to_le_bytes());
                    bytes.extend(core::iter::repeat_n(0u8, data_len));
                    bytes.extend(core::iter::repeat_n(0u8, MAX_PERMITTED_DATA_INCREASE));
                    bytes.extend_from_slice(&0u64.to_le_bytes());
                }
                AccountRecord::Dup { index } => {
                    bytes.push(index);
                    bytes.extend_from_slice(&[0u8; 7]);
                }
            }
        }

        let len = bytes.len();

        // Transfer into 8-byte-aligned Vec<u64>. Round up length.
        let num_u64s = len.div_ceil(8);
        let mut backing: Vec<u64> = vec![0u64; num_u64s];
        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), backing.as_mut_ptr() as *mut u8, len);
        }

        Self {
            backing,
            len,
            record_offsets,
        }
    }

    /// Pointer to the start of the buffer (the `num_accounts` prefix).
    /// Guaranteed 8-byte aligned (backing is `Vec<u64>`).
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.backing.as_mut_ptr() as *mut u8
    }

    /// Access the raw bytes as a mutable slice.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.backing.as_mut_ptr() as *mut u8, self.len) }
    }
}
