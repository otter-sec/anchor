//! Mock `AccountView` scaffold for anchor-lang integration tests.
//!
//! Construct a stack-backed `AccountView` instance without running under the
//! SVM loader. Enables Miri Tree Borrows witnesses for the aliasing patterns
//! anchor-v2 relies on (typed `CpiHandle` + unchecked CPI, `AccountView:
//! Copy` shared state, `Slab::header_ptr` write provenance).
//!
//! ## Usage
//!
//! ```ignore
//! use anchor_lang::testing::AccountBuffer;
//!
//! let mut buf = AccountBuffer::<256>::new();
//! buf.init([1; 32], [0; 32], /*data_len*/ 0, /*is_signer*/ true,
//!          /*is_writable*/ false, /*executable*/ false);
//! let view = unsafe { buf.view() };
//! // Use `view` in tests — e.g. Miri soundness witnesses.
//! ```

use {
    core::cell::UnsafeCell,
    pinocchio::account::{AccountView, RuntimeAccount},
    solana_address::Address,
};

/// Size of the RuntimeAccount header + minimum 8 bytes for data/padding.
pub const MIN_ACCOUNT_BUF: usize = core::mem::size_of::<RuntimeAccount>() + 8;

/// Stack-allocated account buffer. `N` is total buffer size in bytes.
/// Header occupies `size_of::<RuntimeAccount>()` bytes; remainder is
/// available for account data (bounded by `data_len` set in `init`).
///
/// `#[repr(C, align(8))]` matches `RuntimeAccount`'s 8-byte alignment
/// requirement.
///
/// `inner` is wrapped in `UnsafeCell` so that `AccountView` copies handed
/// out by `view()` and the `set_*` mutators can alias the same backing
/// bytes soundly — modelling the real SVM loader, where the runtime's
/// input buffer is shared, externally-mutable memory rather than an
/// exclusively-borrowed Rust allocation. Under Tree Borrows, pointers
/// derived from `UnsafeCell::get()` carry SharedReadWrite tags whose
/// writes don't invalidate sibling aliases; using `&mut self` methods
/// (as in earlier iterations of this scaffold) instead creates sibling
/// Unique tags and a later setter would `Disable` a live `AccountView`'s
/// tag mid-test.
#[repr(C, align(8))]
pub struct AccountBuffer<const N: usize> {
    inner: UnsafeCell<[u8; N]>,
}

impl<const N: usize> AccountBuffer<N> {
    pub fn new() -> Self {
        assert!(
            N >= core::mem::size_of::<RuntimeAccount>(),
            "AccountBuffer<N> needs N >= size_of::<RuntimeAccount>()"
        );
        Self {
            inner: UnsafeCell::new([0u8; N]),
        }
    }

    /// Raw pointer to the header region.
    pub fn raw(&self) -> *mut RuntimeAccount {
        self.inner.get() as *mut RuntimeAccount
    }

    /// Populate the header. `NOT_BORROWED` = 255 (= `NON_DUP_MARKER`)
    /// means the account is ready for mut/immut borrows.
    pub fn init(
        &self,
        address: [u8; 32],
        owner: [u8; 32],
        data_len: usize,
        is_signer: bool,
        is_writable: bool,
        executable: bool,
    ) {
        let raw = self.raw();
        // SAFETY: raw points at a zero-initialized buffer of size N >=
        // size_of::<RuntimeAccount>(), aligned to 8.
        unsafe {
            (*raw).borrow_state = pinocchio::account::NOT_BORROWED;
            (*raw).is_signer = is_signer as u8;
            (*raw).is_writable = is_writable as u8;
            (*raw).executable = executable as u8;
            (*raw).padding = [0u8; 4];
            (*raw).address = Address::new_from_array(address);
            (*raw).owner = Address::new_from_array(owner);
            (*raw).lamports = 100;
            (*raw).data_len = data_len as u64;
        }
    }

    /// Set the account's data bytes (at offset `size_of::<RuntimeAccount>()`
    /// through `+ data_len`). Caller must ensure `init` was called with a
    /// matching `data_len`.
    pub fn write_data(&self, data: &[u8]) {
        let offset = core::mem::size_of::<RuntimeAccount>();
        assert!(
            offset + data.len() <= N,
            "write_data would overflow buffer: offset {} + data.len() {} > N {}",
            offset,
            data.len(),
            N
        );
        // SAFETY: offset + data.len() <= N (checked above); source and
        // destination are distinct (data is a borrowed slice, destination
        // is inside this buffer's UnsafeCell).
        unsafe {
            let dst = (self.inner.get() as *mut u8).add(offset);
            core::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
        }
    }

    /// Read the data region as a byte slice (bounded by data_len in header).
    ///
    /// The returned slice aliases the buffer's `UnsafeCell` contents. Callers
    /// must not issue mutating calls (`set_*`, `write_data`, or writes via a
    /// live `AccountView`) while the slice is alive.
    pub fn read_data(&self) -> &[u8] {
        let offset = core::mem::size_of::<RuntimeAccount>();
        // SAFETY: inner.get() points to a fully-initialized buffer; data_len
        // is a u64 at the `data_len` field of the RuntimeAccount header.
        let data_len = unsafe { (*self.raw()).data_len as usize };
        assert!(offset + data_len <= N, "data_len exceeds buffer");
        // SAFETY: bounds checked above; caller preserves no-concurrent-write
        // discipline per the method contract.
        unsafe {
            let base = self.inner.get() as *const u8;
            core::slice::from_raw_parts(base.add(offset), data_len)
        }
    }

    /// Construct an `AccountView` over this buffer. The buffer must
    /// outlive the view.
    ///
    /// # Safety
    ///
    /// Caller must ensure `init()` was called. The returned `AccountView`
    /// borrows the buffer via a raw pointer — do not drop or move the
    /// `AccountBuffer` while the `AccountView` is live.
    pub unsafe fn view(&self) -> AccountView {
        AccountView::new_unchecked(self.raw())
    }

    /// Direct access to the borrow state byte. Useful for setting up
    /// duplicate-account scenarios where `borrow_state` encodes a dup
    /// index (0..=254) instead of `NOT_BORROWED` (255).
    pub fn set_borrow_state(&self, value: u8) {
        unsafe {
            (*self.raw()).borrow_state = value;
        }
    }

    /// Direct access to the lamports field.
    pub fn set_lamports(&self, value: u64) {
        unsafe {
            (*self.raw()).lamports = value;
        }
    }

    /// Overwrite the `data_len` field in the header. Useful for
    /// exercising post-construction resize scenarios without going
    /// through a full CPI path.
    pub fn set_data_len(&self, value: u64) {
        unsafe {
            (*self.raw()).data_len = value;
        }
    }

    /// Overwrite the `owner` field. Useful for simulating a CPI that
    /// transfers ownership of the account.
    pub fn set_owner(&self, owner: [u8; 32]) {
        unsafe {
            (*self.raw()).owner = Address::new_from_array(owner);
        }
    }
}
