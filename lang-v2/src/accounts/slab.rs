use {
    super::slab_hooks::{SlabInit, SlabSchema},
    crate::{require, AccountInitialize, AnchorAccount, Discriminator, Id},
    bytemuck::{Pod, Zeroable},
    core::{
        marker::PhantomData,
        ops::{Deref, DerefMut, Index, IndexMut},
    },
    pinocchio::{
        account::{AccountView, NOT_BORROWED},
        Address,
    },
    solana_program_error::ProgramError,
};

// SlabSchema / SlabInit (bytes-level hooks Slab invokes on `H`) live in
// `accounts::slab_hooks`. The forwards below tie them + the
// wrapper-level `AccountInitialize` together.

/// Disambiguation for failed owner checks: uninitialized placeholder vs.
/// genuine wrong owner. Used by `SlabSchema`'s blanket impl (via `super::`).
#[inline(always)]
pub(super) fn cold_owner_error(view: &AccountView) -> ProgramError {
    if view.lamports() == 0 && view.owned_by(&crate::programs::System::id()) {
        ProgramError::UninitializedAccount
    } else {
        ProgramError::IllegalOwner
    }
}

/// Error for read-only account passed to `load_mut`.
#[cfg(feature = "guardrails")]
#[inline(always)]
pub(super) fn cold_not_writable() -> ProgramError {
    ProgramError::InvalidAccountData
}

/// Capacity from live `data_len` / `items_offset` / `item_size`. Returns 0
/// when `data_len < items_offset` to guard against underflow if an external
/// `realloc_account` shrinks the buffer while a `Slab` is retained. Caller
/// must ensure `item_size > 0` (`Slab` skips this for ZST items).
///
/// Factored out of `Slab::capacity` as a free `const fn` so the Kani proofs
/// below (`capacity_never_underflows`, `capacity_fits_within_data_len_item_*`,
/// `clamp_never_exceeds_capacity`) can reason about it in isolation from
/// `Slab`'s generic plumbing.
#[inline(always)]
const fn capacity_for(data_len: usize, items_offset: usize, item_size: usize) -> usize {
    if data_len < items_offset {
        0
    } else {
        (data_len - items_offset) / item_size
    }
}

/// `Slab<H, T>` initialization delegates account creation to `H::SlabInit`,
/// then loads the freshly initialized slab.
impl<H, T> AccountInitialize for Slab<H, T>
where
    H: SlabInit + Pod + Zeroable + SlabSchema,
    Self: AnchorAccount,
{
    type Params<'a> = H::Params<'a>;

    #[inline(always)]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        _owner: &Address,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        H::create_and_initialize(payer, account, space, params, signer_seeds)?;
        // SAFETY: `create_and_initialize` just created this account; no other
        // mutable reference to its data can exist yet.
        unsafe { <Self as AnchorAccount>::load_mut_after_init(*account) }
    }
}

/// Forward `Discriminator` from a `Slab<H, _>` to its header type. Lets the
/// `#[account(zeroed)]` derive codegen look up the disc via the field type
/// directly (e.g. `<Account<Counter> as Discriminator>::DISCRIMINATOR`)
/// instead of extracting an inner type by string-matching on "Account".
impl<H, T> Discriminator for Slab<H, T>
where
    H: Discriminator + Pod + Zeroable + SlabSchema,
{
    const DISCRIMINATOR: &'static [u8] = H::DISCRIMINATOR;
}

// Header-only accounts use the schema's minimum size. Tail-carrying Slabs use
// the zero-capacity tail layout (`[header][len]`) as their minimum.
impl<H, T> crate::Space for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    const INIT_SPACE: usize = Slab::<H, T>::MIN_DATA_LEN;
}

// ---------------------------------------------------------------------------
// Slab<H, T>
// ---------------------------------------------------------------------------

/// Generic account type with a typed header `H` and optional trailing
/// length-prefixed array of items `T`.
///
/// ## Layout
///
/// `[disc:8][H]` — when `T` is a ZST (`Account<T> = Slab<T, HeaderOnly>`).
/// `[disc:8][H][len:u32][pad][items...]` — when `T: Pod` (non-ZST).
/// Capacity is derived from `view.data_len()` at load time.
///
/// ## Rent responsibility
///
/// Push/pop/clear/`resize_to_capacity` do **not** touch lamports. Use
/// [`min_lamports`](Slab::min_lamports), [`top_up`](Slab::top_up),
/// [`refund`](Slab::refund), and [`space_for`](Slab::space_for).
///
/// ## Tail-only methods
///
/// `try_push`, `pop`, `clear`, `truncate`, `swap_remove`, `Index<usize>`
/// require `T: Pod`; `HeaderOnly` doesn't impl `Pod`, so these are
/// compile errors on `Account<T>`.
///
/// ## Internals
///
/// Caches a typed pointer to `H` (valid for the instruction lifetime).
/// `is_mutable` gates `DerefMut` to catch missing `#[account(mut)]`.
pub struct Slab<H, T = HeaderOnly>
where
    H: Pod + Zeroable + SlabSchema,
{
    view: AccountView,
    /// Cached pointer to the header (at `HEADER_OFFSET`). Valid for the
    /// entire instruction lifetime (Solana runtime guarantee).
    ///
    /// `len_ptr`, `items_ptr`, and `capacity` are NOT cached here — they're
    /// derived on demand from `header_ptr` + const offsets + `view.data_len()`.
    /// This keeps `Slab` at 3 fields (same footprint as the pre-rewrite
    /// `Account<T>`), so multi-instruction programs don't pay extra stack
    /// frame bytes at every load site.
    header_ptr: *mut H,
    /// Whether this slab was loaded via `load_mut`. Guards `DerefMut` to catch
    /// missing `#[account(mut)]` at the point of access rather than silently
    /// producing UB through a const-provenance pointer.
    is_mutable: bool,
    _tail: PhantomData<T>,
}

impl<H, T> Drop for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    fn drop(&mut self) {
        let borrow_state = unsafe { &mut (*self.view.account_ptr().cast_mut()).borrow_state };
        if self.is_mutable {
            *borrow_state = NOT_BORROWED;
        } else if *borrow_state != 0 && *borrow_state != NOT_BORROWED {
            *borrow_state += 1;
        }
    }
}

/// Marker type for the header-only form of [`Slab`]. Does **not** implement
/// `Pod`, so the tail-only `impl` block (gated on `T: Pod`) never matches —
/// calling `.push()` / `.len()` / `.as_slice()` etc. on an `Account<T>` =
/// `Slab<T, HeaderOnly>` is a compile error with "method not found" rather
/// than a runtime misbehavior.
///
/// Users shouldn't reference this type directly; use the `Account<T>`
/// alias for header-only accounts and `Slab<H, Entry>` for dynamic tails.
pub struct HeaderOnly {
    // Prevents instantiation from outside the crate.
    _private: (),
}

impl<H, T> Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    /// Whether `T` is a non-zero-sized type. Folds to a const at
    /// monomorphization time.
    /// `size_of::<T>()` requires no bounds — works for any `T`, including
    /// `HeaderOnly`.
    const HAS_TAIL: bool = core::mem::size_of::<T>() > 0;

    /// Byte offset of the header. Anchor native types have an 8-byte
    /// discriminator so this is `8`; external types (SPL `Mint` /
    /// `TokenAccount`) have `0` via `H::DATA_OFFSET`.
    const HEADER_OFFSET: usize = H::DATA_OFFSET;

    /// Byte offset of the `len` field (when `HAS_TAIL`).
    const LEN_OFFSET: usize = Self::HEADER_OFFSET + core::mem::size_of::<H>();

    /// Byte offset where items start. Equal to `LEN_OFFSET` when `T` is a
    /// ZST; otherwise `LEN_OFFSET + 4`, rounded up to `align_of::<T>()`.
    const ITEMS_OFFSET: usize = {
        if core::mem::size_of::<T>() > 0 {
            let after_len = Self::LEN_OFFSET + 4;
            let a = core::mem::align_of::<T>();
            (after_len + a - 1) & !(a - 1)
        } else {
            Self::LEN_OFFSET
        }
    };

    const MIN_DATA_LEN: usize = if Self::HAS_TAIL {
        Self::ITEMS_OFFSET
    } else {
        H::MIN_DATA_LEN
    };

    #[inline(always)]
    fn assert_header_alignment() {
        // Solana guarantees account data buffers are 8-byte aligned. Headers
        // demanding stricter alignment, or schemas that place the header at a
        // misaligned offset, would make the cached `header_ptr` invalid.
        const {
            assert!(
                core::mem::align_of::<H>() <= 8,
                "Slab header alignment exceeds Solana's 8-byte account data alignment",
            );
            assert!(
                H::DATA_OFFSET % core::mem::align_of::<H>() == 0,
                "Slab header DATA_OFFSET is not aligned for the header type",
            );
        };
    }

    /// Returns the account's address. Always safe regardless of borrow state.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }

    /// The underlying `AccountView` — provided for CPI callers that need the
    /// raw view.
    #[inline(always)]
    pub fn view(&self) -> &AccountView {
        &self.view
    }

    /// Validate `len <= capacity` for the tail region before we do the
    /// lifetime transmute. Works on `&[u8]` directly — no unsafe, no
    /// alignment concerns (uses `u32::from_le_bytes` on a stack copy).
    #[inline(always)]
    fn validate_tail(data: &[u8]) -> Result<(), ProgramError> {
        if !Self::HAS_TAIL {
            return Ok(());
        }
        let data_len = data.len();
        let capacity = capacity_for(data_len, Self::ITEMS_OFFSET, core::mem::size_of::<T>());
        require!(
            capacity <= u32::MAX as usize,
            ProgramError::InvalidAccountData
        );
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&data[Self::LEN_OFFSET..Self::LEN_OFFSET + 4]);
        let len = u32::from_le_bytes(len_bytes) as usize;
        require!(len <= capacity, ProgramError::InvalidAccountData);
        Ok(())
    }

    #[inline(always)]
    fn from_ref(view: AccountView) -> Result<Self, ProgramError> {
        Self::assert_header_alignment();
        let borrow_state = unsafe { (*view.account_ptr()).borrow_state };
        // 0 is the mutable-borrow sentinel; 1 has no shared-borrow slot left
        // for Slab to register below.
        if borrow_state < 2 {
            return Err(ProgramError::AccountBorrowFailed);
        }
        // SAFETY: AccountView's data pointer is valid for the instruction lifetime
        // (Solana runtime guarantee). The borrow-state check above rejects a live
        // mutable borrow before creating the shared view.
        let data = unsafe { view.borrow_unchecked() };
        H::validate(&view, data)?;
        if data.len() < Self::ITEMS_OFFSET {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Self::validate_tail(data)?;
        let header_ptr = unsafe { view.data_ptr().add(Self::HEADER_OFFSET) } as *mut H;
        // Mark one immutable borrow outstanding so that any copied AccountView
        // cannot obtain a mutable borrow via try_borrow_mut(). Additional
        // immutable borrows are still allowed (safe — they alias &H, not &mut H,
        // and DerefMut panics on a read-only Slab).
        unsafe { (*view.account_ptr().cast_mut()).borrow_state = borrow_state - 1 };
        Ok(Self {
            view,
            header_ptr,
            is_mutable: false,
            _tail: PhantomData,
        })
    }

    /// Low-level constructor: set up `header_ptr` with write provenance,
    /// no validation. Under `guardrails`, includes a minimum-length check.
    #[inline(always)]
    fn build_mutable(view: AccountView) -> Result<Self, ProgramError> {
        Self::assert_header_alignment();
        // SAFETY: AccountView's data pointer is valid for the instruction lifetime.
        // Duplicate mutable accounts are rejected at deserialization.
        let data = unsafe { view.borrow_unchecked() };
        if data.len() < Self::MIN_DATA_LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        // Derive header_ptr through data_mut_ptr to preserve write provenance.
        // Using data_ptr → *const would lose it under Stacked Borrows / Tree Borrows.
        let mut view_mut = view;
        let header_ptr = unsafe { view_mut.data_mut_ptr().add(Self::HEADER_OFFSET) } as *mut H;
        // Mark as mutably borrowed so that any copied AccountView cannot
        // obtain any borrow (immutable or mutable) via try_borrow*().
        // borrow_state == 0 means "exclusively borrowed" in pinocchio's
        // protocol. Slab itself accesses data via borrow_unchecked*() which
        // bypasses this check.
        unsafe { (*view_mut.account_mut_ptr()).borrow_state = 0 };
        Ok(Self {
            view,
            header_ptr,
            is_mutable: true,
            _tail: PhantomData,
        })
    }

    // -----------------------------------------------------------------------
    // Rent helpers — work for both header-only and tail forms.
    // -----------------------------------------------------------------------

    /// Rent-exempt lamport minimum for the account's current data length.
    /// Uses runtime sysvar by default; `const-rent` feature uses baked-in rate.
    #[inline]
    pub fn min_lamports(&self) -> Result<u64, ProgramError> {
        crate::cpi::rent_exempt_lamports(self.view.data_len())
    }

    /// Current size of the account's data region in bytes.
    #[inline(always)]
    pub fn current_space(&self) -> usize {
        self.view.data_len()
    }

    /// Pay the rent shortfall from `payer`. No-op if the account already
    /// holds at least `min_lamports()`.
    ///
    /// Uses a `system::Transfer` CPI; `payer` must be a signer on the outer
    /// transaction (pinocchio enforces signerness at CPI time).
    pub fn top_up(&mut self, payer: &AccountView) -> Result<(), ProgramError> {
        let required = self.min_lamports()?;
        let current = self.view.lamports();
        if current >= required {
            return Ok(());
        }
        let deficit = required - current;
        crate::cpi::transfer_lamports_unchecked(payer, &self.view, deficit)
    }

    /// Move excess lamports (current - min_lamports) from the account to
    /// `recipient`. No-op if the account is already at the rent floor.
    ///
    /// Direct lamport arithmetic, no CPI — callers must only use this for
    /// accounts whose owner permits in-program lamport mutation.
    pub fn refund(&mut self, recipient: &mut AccountView) -> Result<(), ProgramError> {
        let required = self.min_lamports()?;
        let current = self.view.lamports();
        if current <= required {
            return Ok(());
        }
        let excess = current - required;
        let new_recipient = recipient
            .lamports()
            .checked_add(excess)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        recipient.set_lamports(new_recipient);
        let mut self_view = self.view;
        self_view.set_lamports(required);
        Ok(())
    }
}

// ===========================================================================
// Tail-only impl block — `T: Pod` bound excludes `HeaderOnly`, so these
// methods are "method not found" compile errors on `Account<H>`.
// ===========================================================================

impl<H, T> Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
    T: Pod,
{
    // -----------------------------------------------------------------------
    // Safe byte-slice accessors — bounds checks + bytemuck alignment checks
    // trade a small cost for zero unsafe in the tail-mutation path.
    //
    // `Deref<Target = H>` still uses the cached `header_ptr` for zero-cost
    // field access — the hot path for `ctx.accounts.ledger.authority` is
    // unchanged.
    // -----------------------------------------------------------------------

    /// The account data bytes. Always valid for the instruction lifetime.
    #[inline(always)]
    fn guard_bytes(&self) -> &[u8] {
        // SAFETY: AccountView data is valid for the instruction lifetime.
        // Duplicate mutable accounts are rejected at deserialization.
        unsafe { self.view.borrow_unchecked() }
    }

    /// Mutable account data bytes. Panics if the slab was loaded read-only.
    #[inline(always)]
    fn guard_bytes_mut(&mut self) -> &mut [u8] {
        if !self.is_mutable {
            panic!(
                "Slab<H, T> mutated through a read-only load. Add #[account(mut)] to your \
                 accounts struct."
            );
        }
        // SAFETY: is_mutable guarantees this was loaded via load_mut.
        // AccountView data is valid for the instruction lifetime.
        unsafe { self.view.borrow_unchecked_mut() }
    }

    /// Read the `len` field without requiring `LEN_OFFSET` alignment —
    /// `from_le_bytes` operates on a copy, so misaligned layouts are fine.
    #[inline(always)]
    fn read_len(&self) -> u32 {
        let bytes = self.guard_bytes();
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes[Self::LEN_OFFSET..Self::LEN_OFFSET + 4]);
        u32::from_le_bytes(buf)
    }

    /// Write the `len` field. Same alignment-free pattern as `read_len`.
    #[inline(always)]
    fn write_len(&mut self, new_len: u32) {
        let bytes = self.guard_bytes_mut();
        bytes[Self::LEN_OFFSET..Self::LEN_OFFSET + 4].copy_from_slice(&new_len.to_le_bytes());
    }

    /// Total account data size required to hold the header plus `capacity`
    /// items. `const fn`, so callers can put it directly into
    /// `#[account(init, space = Slab::<Ledger, Entry>::space_for(64), ...)]`.
    #[inline(always)]
    pub const fn space_for(capacity: u32) -> usize {
        match Self::try_space_for(capacity) {
            Ok(value) => value,
            Err(_) => panic!("slab account size overflow"),
        }
    }

    /// Fallible runtime version of [`space_for`](Self::space_for). Use this for
    /// user-supplied capacities so arithmetic failures become program errors.
    #[inline(always)]
    pub const fn try_space_for(capacity: u32) -> Result<usize, ProgramError> {
        let item_bytes = match (capacity as usize).checked_mul(core::mem::size_of::<T>()) {
            Some(value) => value,
            None => return Err(ProgramError::ArithmeticOverflow),
        };
        match Self::ITEMS_OFFSET.checked_add(item_bytes) {
            Some(value) => Ok(value),
            None => Err(ProgramError::ArithmeticOverflow),
        }
    }

    /// Current number of items in the tail region.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.read_len() as usize
    }

    /// How many items the account's tail region can currently hold.
    /// Returns 0 if `data_len < ITEMS_OFFSET` (guards against post-resize
    /// underflow when an external `realloc_account` has shrunk the
    /// account below the Slab's structural minimum).
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        capacity_for(
            self.view.data_len(),
            Self::ITEMS_OFFSET,
            core::mem::size_of::<T>(),
        )
    }

    /// Live `len` clamped to current `capacity`. The stored `len` may
    /// exceed `capacity` if an external `realloc_account` shrank the
    /// account after this `Slab` was constructed; mutation paths must
    /// use this value (not the raw `len`) when computing item offsets
    /// or indexing the tail region.
    #[inline(always)]
    fn effective_len(&self) -> usize {
        self.len().min(self.capacity())
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// View the tail region as an immutable slice. Uses `effective_len` so
    /// a post-load external resize (which would leave raw `len > capacity`)
    /// cannot cause an OOB slice read.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        let len = self.effective_len();
        let bytes = self.guard_bytes();
        // `ITEMS_OFFSET` is const-computed to be `align_of::<T>()`-aligned,
        // and Pod requires `size_of` is a multiple of `align_of`, so every
        // per-item offset is aligned. bytemuck will verify this at runtime.
        let items_bytes =
            &bytes[Self::ITEMS_OFFSET..Self::ITEMS_OFFSET + len * core::mem::size_of::<T>()];
        bytemuck::cast_slice(items_bytes)
    }

    /// View the tail region as a mutable slice. Uses `effective_len`;
    /// same post-resize guard as `as_slice`.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let len = self.effective_len();
        let bytes = self.guard_bytes_mut();
        let items_bytes =
            &mut bytes[Self::ITEMS_OFFSET..Self::ITEMS_OFFSET + len * core::mem::size_of::<T>()];
        bytemuck::cast_slice_mut(items_bytes)
    }

    #[inline(always)]
    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.as_slice().iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, T> {
        self.as_mut_slice().iter_mut()
    }

    #[inline(always)]
    pub fn first(&self) -> Option<&T> {
        self.as_slice().first()
    }

    #[inline(always)]
    pub fn last(&self) -> Option<&T> {
        self.as_slice().last()
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }

    #[inline(always)]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.as_mut_slice().get_mut(index)
    }

    // -----------------------------------------------------------------------
    // Tail-region mutations — all safe, go through `guard_bytes_mut()`.
    // -----------------------------------------------------------------------

    /// Append `value` to the tail region.
    ///
    /// Returns `Err(AccountDataTooSmall)` when `len == capacity`. The caller
    /// is responsible for growing the account via `resize_to_capacity`
    /// beforehand.
    pub fn try_push(&mut self, value: T) -> Result<(), ProgramError> {
        let len = self.len();
        if len >= self.capacity() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let new_len = len
            .checked_add(1)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let item_offset = Self::ITEMS_OFFSET + len * core::mem::size_of::<T>();
        {
            let bytes = self.guard_bytes_mut();
            let slot = &mut bytes[item_offset..item_offset + core::mem::size_of::<T>()];
            *bytemuck::from_bytes_mut::<T>(slot) = value;
        }
        self.write_len(new_len);
        Ok(())
    }

    /// Remove and return the last item, or `None` if empty. Uses
    /// `effective_len` so a post-shrink Slab (raw len > capacity) doesn't
    /// read past the live data buffer; the write-back also clamps the
    /// stored len to a value `<= capacity`.
    pub fn pop(&mut self) -> Option<T> {
        let len = self.effective_len();
        if len == 0 {
            return None;
        }
        let new_len = len - 1;
        let item_offset = Self::ITEMS_OFFSET + new_len * core::mem::size_of::<T>();
        let value = {
            let bytes = self.guard_bytes();
            let slot = &bytes[item_offset..item_offset + core::mem::size_of::<T>()];
            *bytemuck::from_bytes::<T>(slot)
        };
        self.write_len(new_len as u32);
        Some(value)
    }

    /// Truncate the tail to `new_len`. Uses `effective_len` so a
    /// post-shrink Slab is brought back to a consistent state: the stored
    /// len ends up at `min(new_len, effective_len)`.
    pub fn truncate(&mut self, new_len: usize) {
        let target = new_len.min(self.effective_len());
        if target != self.len() {
            self.write_len(target as u32);
        }
    }

    /// Clear the tail region (set `len` to 0). Does not zero item memory.
    pub fn clear(&mut self) {
        self.write_len(0);
    }

    /// Swap the item at `index` with the last, then pop. `O(1)` remove.
    /// Uses `effective_len` so a post-shrink Slab can't index past the
    /// live data buffer.
    ///
    /// # Panics
    ///
    /// Panics if `index >= effective_len()`, matching `Vec::swap_remove`.
    pub fn swap_remove(&mut self, index: usize) -> T {
        let len = self.effective_len();
        assert!(index < len, "swap_remove index out of bounds");
        let new_len = len - 1;
        // `as_mut_slice()` returns a bounds-checked `&mut [T]` of length
        // `effective_len`, so `index` and `new_len` are both in-bounds.
        let removed = {
            let items = self.as_mut_slice();
            let value = items[index];
            if index != new_len {
                items[index] = items[new_len];
            }
            value
        };
        self.write_len(new_len as u32);
        removed
    }

    /// Resize the account's data region to hold `new_capacity` items without
    /// touching lamports. Compose with `top_up` / `refund` afterward to
    /// settle rent. Re-derives `header_ptr` after the resize; `guard_bytes*`
    /// pick up the new size from `view.data_len()` automatically.
    pub fn resize_to_capacity(&mut self, new_capacity: u32) -> Result<(), ProgramError> {
        use pinocchio::Resize;

        let new_space = Self::try_space_for(new_capacity)?;
        let mut view_mut = self.view;
        // SAFETY: Slab owns exclusive access to the data (enforced by the
        // borrow flag set in build_mutable). Use resize_unchecked to bypass
        // pinocchio's check_borrow_mut() which would see our flag and fail.
        unsafe { view_mut.resize_unchecked(new_space)? };
        // Re-derive header_ptr with write provenance in case the runtime
        // relocated the buffer.
        self.header_ptr = unsafe { view_mut.data_mut_ptr().add(Self::HEADER_OFFSET) } as *mut H;
        // Clamp len down if we shrunk below the current item count.
        let new_cap = new_capacity as usize;
        if self.len() > new_cap {
            self.write_len(new_capacity);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AnchorAccount / Deref / Index / AsRef impls
// ---------------------------------------------------------------------------

impl<H, T> AnchorAccount for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    type Data = H;
    const MIN_DATA_LEN: usize = Slab::<H, T>::MIN_DATA_LEN;

    #[inline(always)]
    fn load(view: AccountView) -> Result<Self, ProgramError> {
        Self::from_ref(view)
    }

    /// # Safety
    ///
    /// See [`AnchorAccount::load_mut`] — caller must ensure no other live
    /// `&mut` to the same account data exists.
    #[inline(always)]
    unsafe fn load_mut(view: AccountView) -> Result<Self, ProgramError> {
        // Reuses the post-init primitive for construction, then layers full
        // validation on top.
        let slab = Self::load_mut_after_init(view)?;
        // SAFETY: build_mutable succeeded, so the data pointer is valid.
        let data: &[u8] = unsafe { slab.view.borrow_unchecked() };
        H::validate(&slab.view, data)?;
        if data.len() < Self::ITEMS_OFFSET {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Self::validate_tail(data)?;
        Ok(slab)
    }

    /// Fast-path `load_mut` after `create_and_initialize`. Skips
    /// `H::validate` and `validate_tail` (all tautologies post-init).
    ///
    /// # Safety
    ///
    /// See [`AnchorAccount::load_mut`] — no other live `&mut` to the
    /// same account data.
    #[inline(always)]
    unsafe fn load_mut_after_init(view: AccountView) -> Result<Self, ProgramError> {
        // Guardrail: catches "forgot `#[account(mut)]`" early with a clear
        // error. Under `default-features = false` the Solana runtime still
        // rejects the tx when we try to write, just with a less specific
        // message. Compiled out without guardrails.
        #[cfg(feature = "guardrails")]
        if !view.is_writable() {
            return Err(cold_not_writable());
        }
        Self::build_mutable(view)
    }

    #[inline(always)]
    fn account(&self) -> &AccountView {
        &self.view
    }

    fn close(&mut self, mut destination: AccountView) -> pinocchio::ProgramResult {
        let mut self_view = self.view;
        let dest_lamports = destination
            .lamports()
            .checked_add(self_view.lamports())
            .ok_or(ProgramError::ArithmeticOverflow)?;
        destination.set_lamports(dest_lamports);
        self_view.set_lamports(0);

        // Defense-in-depth: scrub the discriminator to [u8::MAX; 8] before
        // pinocchio's close() zeros the 48-byte header (data is left
        // untouched until SVM end-of-instruction zero). If a future caller
        // restores data_len + owner without going through SVM zero-on-
        // allocate, the stale discriminator would otherwise allow a reload
        // with pre-close state. Mirrors `BorshAccount::close`. Gated on
        // `HEADER_OFFSET >= 8` so SPL-owned schemas (Mint / TokenAccount,
        // `DATA_OFFSET = 0`, no discriminator) don't have their first 8
        // bytes corrupted.
        if Self::HEADER_OFFSET >= 8 {
            // SAFETY: `close` requires the same exclusive account-data access
            // as `DerefMut`; generated callers only emit it for mutable
            // accounts. The view's data is valid for the instruction lifetime.
            let data = unsafe { self_view.borrow_unchecked_mut() };
            if data.len() >= 8 {
                data[..8].copy_from_slice(&[u8::MAX; 8]);
            }
        }

        // Flip is_mutable so any post-close DerefMut / tail-mutation panics
        // loudly instead of silently writing through the cached header_ptr
        // to memory pinocchio is about to mark closed. Mirrors how
        // `BorshAccount::close` transitions its borrow guard to Released.
        self.is_mutable = false;

        // SAFETY: Slab owns exclusive access (borrow flag is set). Use
        // close_unchecked to bypass pinocchio's is_borrowed() check.
        unsafe { self_view.close_unchecked() };
        Ok(())
    }
}

impl<H, T> crate::AccountRealloc for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    #[inline(always)]
    fn realloc_account(
        &mut self,
        new_space: usize,
        payer: AccountView,
        zero: bool,
    ) -> pinocchio::ProgramResult {
        let mut view = *self.account();
        if new_space != view.data_len() {
            if new_space < Self::ITEMS_OFFSET {
                return Err(ProgramError::AccountDataTooSmall);
            }
            crate::realloc_account(&mut view, new_space, &payer, zero)?;
            self.header_ptr = unsafe { view.data_mut_ptr().add(Self::HEADER_OFFSET) } as *mut H;
            if Self::HAS_TAIL {
                let new_cap =
                    capacity_for(new_space, Self::ITEMS_OFFSET, core::mem::size_of::<T>());
                if new_cap > u32::MAX as usize {
                    return Err(ProgramError::InvalidArgument);
                }
                let data = unsafe { view.borrow_unchecked_mut() };
                let mut len_bytes = [0u8; 4];
                len_bytes.copy_from_slice(&data[Self::LEN_OFFSET..Self::LEN_OFFSET + 4]);
                let len = u32::from_le_bytes(len_bytes) as usize;
                if len > new_cap {
                    data[Self::LEN_OFFSET..Self::LEN_OFFSET + 4]
                        .copy_from_slice(&(new_cap as u32).to_le_bytes());
                }
            }
        }
        Ok(())
    }
}

impl<H, T> Deref for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    type Target = H;

    #[inline(always)]
    fn deref(&self) -> &H {
        // SAFETY: header_ptr is valid for the instruction lifetime (Solana
        // runtime guarantee). Duplicate mutable accounts are rejected at
        // deserialization, so no aliasing can occur.
        unsafe { &*self.header_ptr }
    }
}

impl<H, T> DerefMut for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut H {
        // Always checked (not guardrails-gated): creating `&mut H` from a
        // const-provenance pointer is UB, so this must run even in release.
        if !self.is_mutable {
            panic!(
                "Slab<H, T> mutably dereferenced but loaded read-only. Add #[account(mut)] to \
                 your accounts struct."
            );
        }
        // SAFETY: is_mutable guarantees header_ptr was derived via data_mut_ptr
        // (write provenance). No other live mutable borrow exists; we hold &mut self.
        unsafe { &mut *self.header_ptr }
    }
}

impl<H, T> crate::ToCpiHandle for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        crate::AnchorAccount::cpi_handle(self)
    }
}

impl<H, T> crate::ToCpiHandleMut for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandleMut<'_>, solana_program_error::ProgramError> {
        crate::AnchorAccount::try_cpi_handle_mut(self)
    }
}

// `T: Pod` bound matches the tail-only impl block — only reachable for
// `Slab<H, T>` where `T` is a real pod type, not `HeaderOnly`.
impl<H, T> Index<usize> for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
    T: Pod,
{
    type Output = T;

    #[inline(always)]
    fn index(&self, index: usize) -> &T {
        &self.as_slice()[index]
    }
}

impl<H, T> IndexMut<usize> for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
    T: Pod,
{
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.as_mut_slice()[index]
    }
}

impl<H, T> AsRef<AccountView> for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    #[inline(always)]
    fn as_ref(&self) -> &AccountView {
        &self.view
    }
}

impl<H, T> AsRef<Address> for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema,
{
    #[inline(always)]
    fn as_ref(&self) -> &Address {
        self.view.address()
    }
}

#[doc(hidden)]
impl<H, T> crate::IdlAccountType for Slab<H, T>
where
    H: Pod + Zeroable + SlabSchema + crate::IdlAccountType,
{
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = H::__IDL_ACCOUNT_ENTRY;
    const __IDL_TYPE_DEF: Option<&'static str> = H::__IDL_TYPE_DEF;
    fn __register_idl_deps(
        accounts: &mut ::alloc::vec::Vec<&'static str>,
        types: &mut ::alloc::vec::Vec<&'static str>,
    ) {
        H::__register_idl_deps(accounts, types);
    }
}

// ---------------------------------------------------------------------------
// Borrow-state registration tests.
//
// `from_ref` / `build_mutable` mark the account's pinocchio borrow_state so
// that a copied `AccountView` cannot escape into a raw mutable byte slice
// while a typed wrapper is alive — otherwise `Slab::Deref{,Mut}` would
// produce `&H` / `&mut H` aliasing a `RefMut<'_, [u8]>` from a *safe*
// `try_borrow_mut()` call on the copy.
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "testing"))]
mod tests {
    use {
        super::*,
        crate::{testing::AccountBuffer, AnchorAccount, Discriminator, Owner},
        bytemuck::{Pod, Zeroable},
        solana_program_error::ProgramError,
    };

    const PROGRAM_ID: [u8; 32] = [0x42; 32];

    #[repr(C)]
    #[derive(Clone, Copy, Pod, Zeroable)]
    struct Counter {
        value: u64,
        _pad: [u8; 8],
    }

    impl Owner for Counter {
        const OWNER: Address = Address::new_from_array(PROGRAM_ID);
    }

    impl Discriminator for Counter {
        const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
    }

    type CounterAccount = Slab<Counter, HeaderOnly>;

    fn setup(buf: &mut AccountBuffer<256>, writable: bool) {
        let data_len = 8 + core::mem::size_of::<Counter>();
        buf.init([0xAA; 32], PROGRAM_ID, data_len, false, writable, false);
        let mut data = [0u8; 24];
        data[..8].copy_from_slice(Counter::DISCRIMINATOR);
        data[8..16].copy_from_slice(&42u64.to_le_bytes());
        buf.write_data(&data);
    }

    #[test]
    fn read_only_load_blocks_try_borrow_mut_on_copy() {
        let mut buf = AccountBuffer::<256>::new();
        setup(&mut buf, false);
        let view = unsafe { buf.view() };

        let acct = CounterAccount::load(view).unwrap();
        assert_eq!(acct.value, 42);

        let mut view_copy = view;
        assert_eq!(
            view_copy.try_borrow_mut().err(),
            Some(ProgramError::AccountBorrowFailed),
            "try_borrow_mut on a view copy must fail while a typed Slab is alive"
        );

        drop(acct);
    }

    #[test]
    fn read_only_load_allows_try_borrow_on_copy() {
        let mut buf = AccountBuffer::<256>::new();
        setup(&mut buf, false);
        let view = unsafe { buf.view() };

        let acct = CounterAccount::load(view).unwrap();

        let view_copy = view;
        assert!(
            view_copy.try_borrow().is_ok(),
            "try_borrow on a view copy should succeed alongside a read-only Slab"
        );

        drop(acct);
    }

    #[test]
    fn mut_load_blocks_all_borrows_on_copy() {
        let mut buf = AccountBuffer::<256>::new();
        setup(&mut buf, true);
        let view = unsafe { buf.view() };

        // SAFETY: this is the only live wrapper over `view`'s data.
        let mut acct = unsafe { CounterAccount::load_mut(view).unwrap() };
        acct.value = 99;

        let mut view_copy = view;
        assert_eq!(
            view_copy.try_borrow_mut().err(),
            Some(ProgramError::AccountBorrowFailed),
            "try_borrow_mut on a view copy must fail while a mutable Slab is alive"
        );
        assert_eq!(
            view_copy.try_borrow().err(),
            Some(ProgramError::AccountBorrowFailed),
            "try_borrow on a view copy must fail while a mutable Slab is alive"
        );

        drop(acct);
    }

    #[test]
    fn mut_load_releases_borrow_state_on_drop() {
        let mut buf = AccountBuffer::<256>::new();
        setup(&mut buf, true);
        let view = unsafe { buf.view() };

        {
            // SAFETY: this is the only live wrapper over `view`'s data.
            let mut acct = unsafe { CounterAccount::load_mut(view).unwrap() };
            acct.value = 99;
        }

        let mut view_copy = view;
        assert!(
            view_copy.try_borrow_mut().is_ok(),
            "dropping a mutable Slab must release the manual borrow marker"
        );
    }

    #[test]
    fn mut_load_blocks_safe_read_only_reload_on_copy() {
        let mut buf = AccountBuffer::<256>::new();
        setup(&mut buf, true);
        let view = unsafe { buf.view() };

        // SAFETY: this is the only live wrapper over `view`'s data.
        let mut acct = unsafe { CounterAccount::load_mut(view).unwrap() };
        acct.value = 99;

        let view_copy = view;
        assert_eq!(
            CounterAccount::load(view_copy).err(),
            Some(ProgramError::AccountBorrowFailed),
            "safe read-only Slab load must not alias a live mutable Slab"
        );
        assert_eq!(acct.value, 99);
    }
}

// Kani proofs for the `capacity_for` helper, which backs `Slab::capacity`
// and therefore every slice-bounds computation in the tail-region
// accessors (`as_slice`, `as_mut_slice`, push/pop/swap_remove). Proving
// these invariants on the free function means any regression to the
// production formula (e.g. replacing the `data_len < items_offset` guard
// with a raw subtract) will break CBMC/Z3's proof obligations.
#[cfg(kani)]
mod kani_proofs {
    use super::capacity_for;

    // Generous bounds on the symbolic inputs — well above any realistic
    // Slab account, but tight enough for CBMC/Z3 to converge.
    const DATA_LEN_MAX: usize = 1 << 16;
    const OFFSET_MAX: usize = 1024;
    const ITEM_SIZE_MAX: usize = 1024;

    // `capacity_for` never panics or wraps for any valid input.
    #[kani::proof]
    fn capacity_never_underflows() {
        let data_len: usize = kani::any();
        let items_offset: usize = kani::any();
        let item_size: usize = kani::any();
        kani::assume(item_size > 0);
        kani::assume(data_len <= DATA_LEN_MAX);
        kani::assume(items_offset <= OFFSET_MAX);
        kani::assume(item_size <= ITEM_SIZE_MAX);
        let _ = capacity_for(data_len, items_offset, item_size);
    }

    // Normal case: `items_offset + capacity * item_size <= data_len` —
    // the invariant `as_slice`'s slice-bounds depends on.
    //
    // A single symbolic-`item_size` harness would be ideal, but the Linux
    // Kani 0.67.0 bundle's CBMC aborts during bitvector encoding on
    // symbolic `usize × usize` multiplication (macOS bundle is fine). The
    // abort surfaces as an unparseable `ERROR` status that panics
    // `kani-driver` before Z3 sees the goal, so `#[kani::solver(z3)]`
    // can't rescue it. Splitting per-concrete `item_size` turns the
    // multiplication into linear arithmetic CBMC dispatches trivially;
    // the covered set spans every Pod width the codebase uses (u8 → u128,
    // Pubkey = 32) plus powers of two up to 1024.
    macro_rules! capacity_fits_within_data_len_for {
        ($name:ident, $item_size:expr) => {
            #[kani::proof]
            fn $name() {
                const ITEM_SIZE: usize = $item_size;
                let data_len: usize = kani::any();
                let items_offset: usize = kani::any();
                kani::assume(data_len <= DATA_LEN_MAX);
                kani::assume(items_offset <= OFFSET_MAX);
                kani::assume(data_len >= items_offset);
                let capacity = capacity_for(data_len, items_offset, ITEM_SIZE);
                assert!(items_offset + capacity * ITEM_SIZE <= data_len);
            }
        };
    }
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_1, 1);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_2, 2);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_4, 4);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_8, 8);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_16, 16);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_32, 32);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_64, 64);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_128, 128);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_256, 256);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_512, 512);
    capacity_fits_within_data_len_for!(capacity_fits_within_data_len_item_1024, 1024);

    // The `.min(capacity)` clamp in `Slab::as_slice`'s `effective_len`
    // never exceeds `capacity`, regardless of the raw stored `len` (which
    // may exceed `capacity` when a retained Slab's buffer has been shrunk
    // externally via `realloc_account`).
    //
    // Combined with `capacity_fits_within_data_len_for!(…)`:
    //   `clamped_len ≤ capacity`
    //   ∧  `items_offset + capacity * item_size ≤ data_len`
    //   ⟹  `items_offset + clamped_len * item_size ≤ data_len`
    // — the slice-bounds invariant that `as_slice` depends on.
    #[kani::proof]
    fn clamp_never_exceeds_capacity() {
        let capacity: usize = kani::any();
        let raw_len: usize = kani::any();
        kani::assume(capacity <= DATA_LEN_MAX);
        kani::assume(raw_len <= DATA_LEN_MAX);
        let clamped_len = if raw_len < capacity {
            raw_len
        } else {
            capacity
        };
        assert!(clamped_len <= capacity);
    }
}
