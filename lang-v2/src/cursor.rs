//! Lazy cursor over the serialized instruction input buffer.
//!
//! [`AccountCursor`] yields `AccountView`s on demand so dispatch can
//! happen first and each arm walks only its declared accounts. A
//! caller-provided `lookup` array resolves duplicate account references:
//! when the BPF loader writes a dup index into `borrow_state`, the cursor
//! returns the earlier `AccountView` from `lookup[idx]`. Every view of one
//! account therefore points at the same `RuntimeAccount` header and shares
//! its `borrow_state` byte, which the account wrappers use to reject
//! conflicting borrows of an aliased account.

use pinocchio::account::{AccountView, RuntimeAccount, MAX_PERMITTED_DATA_INCREASE};

/// Sentinel value in the serialized `borrow_state` byte indicating a
/// non-duplicated account. Any other value (0..=254) indicates a
/// duplicate and holds the index of the earlier account it aliases.
pub const NON_DUP_MARKER: u8 = u8::MAX;

/// Static (fixed) per-account size in the serialized input buffer: the
/// `RuntimeAccount` header + the `MAX_PERMITTED_DATA_INCREASE` padding
/// region that trails the account data.
const STATIC_ACCOUNT_DATA: usize =
    core::mem::size_of::<RuntimeAccount>() + MAX_PERMITTED_DATA_INCREASE;

/// 8-byte alignment required between account records on BPF.
const BPF_ALIGN_OF_U128: usize = 8;

/// Cursor into the serialized instruction input buffer.
///
/// Advances a raw pointer past one account record per [`next`](AccountCursor::next)
/// call and uses a `lookup` array for dup resolution.
///
/// # Safety
///
/// Created from the runtime's input pointer (r1). Must not outlive the
/// entrypoint invocation. Callers must ensure:
///
/// - `lookup` points to `[AccountView; N]` where `N >= max(consumed + 1,
///   max_dup_index + 1)`. In practice: a `[MaybeUninit<AccountView>; 256]`
///   allocated once in the dispatcher frame.
/// - `next()` is called fewer than `num_accounts` times.
pub struct AccountCursor {
    /// Current position in the input buffer. Advances on each `next()`.
    ptr: *mut u8,

    /// Pointer to the caller's `[AccountView; N]` lookup array.
    /// Indexed by `consumed` on write and by the serialized dup index
    /// on read (for duplicate resolution).
    lookup: *mut AccountView,

    /// Number of accounts yielded so far. Used both as the write index
    /// into `lookup` and as a runtime counter exposed to callers for
    /// bookkeeping (e.g., remaining-accounts walks).
    consumed: u8,
}

impl AccountCursor {
    /// Create a fresh cursor at the start of the serialized accounts
    /// region. `input_ptr` must point at the 8-byte `num_accounts`
    /// length prefix in the input buffer (i.e. the runtime-provided `r1`
    /// value); the cursor advances past it internally.
    ///
    /// # Safety
    ///
    /// See type-level safety notes.
    #[inline(always)]
    pub unsafe fn new(input_ptr: *mut u8, lookup: *mut AccountView) -> Self {
        Self {
            ptr: input_ptr.add(core::mem::size_of::<u64>()),
            lookup,
            consumed: 0,
        }
    }

    /// Number of accounts yielded from this cursor so far.
    #[inline(always)]
    pub fn consumed(&self) -> u8 {
        self.consumed
    }

    /// Walk N accounts in a tight loop, storing views in the lookup array.
    /// Returns a slice of the walked views.
    /// This avoids interleaving cursor math with validation logic, letting
    /// LLVM optimize the walk loop.
    ///
    /// # Safety
    ///
    /// Caller must ensure N does not exceed the remaining accounts.
    #[inline(always)]
    pub unsafe fn walk_n(&mut self, n: usize) -> &[AccountView] {
        let start = self.consumed as usize;
        for _ in 0..n {
            self.next();
        }
        core::slice::from_raw_parts(self.lookup.add(start), n)
    }

    /// Advance past one account record and return its `AccountView`.
    ///
    /// Handles both non-duplicated accounts (walks past the record
    /// header + data + padding) and duplicated accounts (reads the
    /// earlier view from `lookup`).
    ///
    /// Also writes the resolved view back into `lookup[consumed]` so
    /// future dup references resolve correctly, then increments
    /// `consumed`.
    ///
    /// # Safety
    ///
    /// Must not be called if `consumed` has already reached the
    /// transaction's total `num_accounts` — there's no trailing account
    /// record at that point. The caller (the derive-generated
    /// dispatcher or a user-level `remaining_accounts()` walk) is
    /// responsible for checking this upfront.
    #[inline(always)]
    pub unsafe fn next(&mut self) -> AccountView {
        let account: *mut RuntimeAccount = self.ptr as *mut RuntimeAccount;

        // Advance 8 bytes at the head of every slot: covers the
        // rent_epoch trailer for non-dup slots, or the full
        // (dup_marker + 7 bytes padding) body of dup slots. Pinocchio's
        // `read_account` applies the same "out-of-order" advance — it's
        // algebraically equivalent to adding the struct size + data +
        // padding + alignment at the end.
        self.ptr = self.ptr.add(core::mem::size_of::<u64>());

        // First account (consumed == 0) can never be a duplicate —
        // short-circuits the dup check for the first field.
        let borrow_state = (*account).borrow_state;
        let view = if self.consumed == 0 || borrow_state == NON_DUP_MARKER {
            // Non-dup: write data_len into the padding slot so
            // `AccountView::resize()` can enforce
            // MAX_PERMITTED_DATA_INCREASE later without another syscall.
            (*account).padding = u32::to_le_bytes((*account).data_len as u32);
            let data_len = (*account).data_len as usize;
            self.ptr = self.ptr.add(STATIC_ACCOUNT_DATA);
            self.ptr = self.ptr.add(data_len);
            // Align to the next 8-byte boundary. Use strict provenance APIs to
            // allow this to be tested under Miri.
            let addr = self.ptr.addr();
            let aligned = (addr + (BPF_ALIGN_OF_U128 - 1)) & !(BPF_ALIGN_OF_U128 - 1);
            self.ptr = self.ptr.add(aligned - addr);
            AccountView::new_unchecked(account)
        } else {
            // Duplicate: look up the earlier slot. Safe because the
            // runtime only emits dup indices that are strictly less
            // than the current `consumed`, so the slot is already
            // populated by a prior `next()` call.
            *self.lookup.add(borrow_state as usize)
        };

        // Record this view so later dup references can resolve it.
        *self.lookup.add(self.consumed as usize) = view;
        self.consumed = self.consumed.wrapping_add(1);
        view
    }
}
