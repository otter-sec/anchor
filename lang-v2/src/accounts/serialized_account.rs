use {
    crate::{require, require_eq, AccountInitialize, AnchorAccount, Discriminator, Owner},
    core::{
        marker::PhantomData,
        ops::{Deref, DerefMut},
    },
    pinocchio::{
        account::{AccountView, Ref, RefMut},
        address::Address,
    },
    solana_program_error::ProgramError,
    solana_program_memory::sol_memset,
};

/// Discriminator length in bytes. All `#[account]` types use an 8-byte
/// discriminator; serialized accounts prefix their data with it.
pub(crate) const DISC_LEN: usize = 8;

/// Pluggable codec for [`SerializedAccount`].
///
/// Implementors are zero-sized tag types (e.g. [`super::BorshSerializer`])
/// chosen at the use site. The trait is generic over `T` rather than carrying
/// an associated `Value`, so a single tag can encode every `T` whose own
/// bounds the impl satisfies.
///
/// Both methods operate on a slice cursor (`&mut &[u8]` / `&mut &mut [u8]`)
/// and must advance it by the bytes they consume. `SerializedAccount` uses
/// that cursor position to scrub stale bytes when a value serializes shorter
/// than it did at load time.
pub trait AnchorAccountSerialize<T> {
    /// Serialize `value` into the buffer past the discriminator.
    fn serialize(value: &T, buf: &mut &mut [u8]) -> Result<(), ProgramError>;

    /// Deserialize a `T` from the buffer past the discriminator.
    fn deserialize(buf: &mut &[u8]) -> Result<T, ProgramError>;
}

/// Account whose payload is encoded by a pluggable serializer `S`.
///
/// Validates owner, checks discriminator, deserializes via `S`. Holds a
/// pinocchio borrow guard (`Ref` for `load`, `RefMut` for `load_mut`);
/// `exit()` serializes through the held `RefMut`.
///
/// The wincode-backed instantiation is exposed as [`super::BorshAccount<T>`].
///
/// ## `#[account(owner = X @ MyErr)]` does NOT surface `MyErr`
///
/// Owner/discriminator validation runs inside `load`/`load_mut`, before
/// derive-level constraints. A mismatch is `ProgramError::IllegalOwner`,
/// not the user's `@ MyErr`. For a custom error, use `UncheckedAccount`
/// with derive-level `owner = X @ MyErr` (you lose the built-in disc /
/// codec checks).
pub struct SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    view: AccountView,
    data: T,
    borrow: SerializedAccountBorrow,
    serialized_len: usize,
    _serializer: PhantomData<S>,
}

enum SerializedAccountBorrow {
    Immutable { _guard: Ref<'static, [u8]> },
    Mutable { guard: RefMut<'static, [u8]> },
    Released,
}

// Forward `Space::INIT_SPACE` from the inner type and add 8 for the
// discriminator. Lets `#[account(init)]` default to the correct size
// when `space` is omitted.
impl<T, S> crate::Space for SerializedAccount<T, S>
where
    T: Owner + Discriminator + crate::Space,
    S: AnchorAccountSerialize<T>,
{
    const INIT_SPACE: usize = 8 + T::INIT_SPACE;
}

impl<T, S> SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    /// Returns the account's on-chain address. Inherent method so
    /// `.address()` works uniformly on all wrapper types — `Signer`,
    /// `Account<T>`, `BorshAccount<T>`, `UncheckedAccount`, etc. — without
    /// callers needing to know whether the wrapper derefs to `AccountView`
    /// or to `T`.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }

    /// Commit `self.data` to the buffer and release the borrow guard so
    /// the underlying `AccountView` can be resized or passed to CPIs
    /// that call `check_borrow_mut()`. The CPI sees the user's
    /// in-memory mutations because they were just serialized. After
    /// this, `exit()` becomes a no-op until `reacquire_borrow_mut()` is
    /// called. Immutable / already-released borrows skip the commit.
    pub fn release_borrow(&mut self) -> Result<(), ProgramError> {
        Self::serialize_mutable_borrow(
            &self.data,
            &mut self.borrow,
            &mut self.serialized_len,
        )?;
        self.borrow = SerializedAccountBorrow::Released;
        Ok(())
    }

    fn serialize_mutable_borrow(
        data: &T,
        borrow: &mut SerializedAccountBorrow,
        serialized_len: &mut usize,
    ) -> Result<(), ProgramError> {
        if let SerializedAccountBorrow::Mutable { ref mut guard } = borrow {
            let payload_len = guard.len() - DISC_LEN;
            let mut payload = &mut guard[DISC_LEN..];
            S::serialize(data, &mut payload)?;

            let new_serialized_len = payload_len - payload.len();
            let zero_len = serialized_len
                .saturating_sub(new_serialized_len)
                .min(payload.len());
            unsafe { sol_memset(&mut payload[..zero_len], 0, zero_len) };
            *serialized_len = new_serialized_len;
        }
        Ok(())
    }

    /// Re-acquire a mutable borrow after a `release_borrow()` + CPI.
    ///
    /// Re-runs the full load-time invariants (owner / size /
    /// discriminator) and re-deserializes `self.data` from the live
    /// buffer so CPI-induced changes are picked up and a CPI that
    /// reassigned the account or swapped its disc is rejected.
    /// `release_borrow` already committed the user's pre-CPI
    /// modifications to the buffer, so the re-deserialized state
    /// reflects {pre-CPI mutations} ∪ {CPI mutations}.
    ///
    /// Returns `IllegalOwner` / `AccountDataTooSmall` /
    /// `InvalidAccountData` if the account no longer validates as `T`.
    pub fn reacquire_borrow_mut(&mut self) -> Result<(), ProgramError> {
        // Re-run the load-time invariants. A CPI in the release window
        // could have mutated owner, discriminator, or payload in any
        // combination — without re-checking, we'd accept an account that
        // no longer validates as `T`.
        require!(self.view.owned_by(&T::OWNER), ProgramError::IllegalOwner);
        let mut view_mut = self.view;
        let data_ref = view_mut.try_borrow_mut()?;
        if data_ref.len() < DISC_LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        require_eq!(
            &data_ref[..DISC_LEN],
            T::DISCRIMINATOR,
            ProgramError::InvalidAccountData
        );
        let (data, serialized_len) = Self::deserialize_payload_with_len(&data_ref[DISC_LEN..])?;
        self.data = data;
        self.serialized_len = serialized_len;
        let guard: RefMut<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        self.borrow = SerializedAccountBorrow::Mutable { guard };
        Ok(())
    }

    /// Refresh only the borrow guard without touching `self.data`. Used
    /// after a scope-local buffer resize — the `realloc` account
    /// constraint emits this after `realloc_account`, and `exit()`'s
    /// stale-size path calls it too — where the user's in-memory
    /// modifications to `self.data` must be preserved for the subsequent
    /// serialize. Re-deserializing here would read the pre-resize bytes
    /// still on disk, which fails in the shrink case where the stored
    /// length prefix now exceeds the post-resize buffer.
    ///
    /// For post-CPI use (where CPI may have mutated owner, disc, or
    /// payload), use [`reacquire_borrow_mut`] instead.
    pub fn reacquire_guard_only(&mut self) -> Result<(), ProgramError> {
        let mut view_mut = self.view;
        let data_ref = view_mut.try_borrow_mut()?;
        // A resize that left the buffer shorter than the discriminator
        // means the on-chain `T` discriminator was just truncated. Reject
        // here so the realloc gets rolled back rather than leaving the
        // account permanently un-loadable (and panicking exit() at
        // `guard[DISC_LEN..]`).
        if data_ref.len() < DISC_LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let guard: RefMut<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        self.borrow = SerializedAccountBorrow::Mutable { guard };
        Ok(())
    }

    fn deserialize_payload_with_len(payload: &[u8]) -> Result<(T, usize), ProgramError> {
        let payload_len = payload.len();
        let mut cursor = payload;
        let data = S::deserialize(&mut cursor)?;
        Ok((data, payload_len - cursor.len()))
    }

    fn validate_and_load(
        view: AccountView,
        data: &[u8],
    ) -> Result<(T, usize), ProgramError> {
        // Hot path: a single owner check. The "uninitialized placeholder"
        // disambiguation lives in `cold_owner_error` (slab.rs) — see
        // the comment there for why this is safe.
        require!(view.owned_by(&T::OWNER), super::slab::cold_owner_error(&view));
        if data.len() < DISC_LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        require_eq!(
            &data[..DISC_LEN],
            T::DISCRIMINATOR,
            ProgramError::InvalidAccountData
        );
        Self::deserialize_payload_with_len(&data[DISC_LEN..])
    }
}

impl<T, S> AnchorAccount for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    type Data = T;
    const MIN_DATA_LEN: usize = 8;

    fn load(view: AccountView) -> Result<Self, ProgramError> {
        let data_ref = view.try_borrow()?;
        let (data, serialized_len) = Self::validate_and_load(view, &data_ref)?;
        // SAFETY: AccountView's raw pointer is valid for the entire instruction
        // lifetime (Solana runtime guarantee). We hold the Ref to prevent
        // subsequent mutable borrows on the same account (duplicate detection).
        let guard: Ref<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        Ok(Self {
            view,
            data,
            borrow: SerializedAccountBorrow::Immutable { _guard: guard },
            serialized_len,
            _serializer: PhantomData,
        })
    }

    /// # Safety
    ///
    /// See [`AnchorAccount::load_mut`] — caller must ensure no other live
    /// `&mut` to the same account data exists.
    unsafe fn load_mut(view: AccountView) -> Result<Self, ProgramError> {
        // Guardrail: catches "forgot `#[account(mut)]`" early with a clear
        // error. Under `default-features = false` the Solana runtime still
        // rejects the tx when we try to write, just with a less specific
        // message. Zero CU when compiled out.
        #[cfg(feature = "guardrails")]
        if !view.is_writable() {
            return Err(super::slab::cold_not_writable());
        }
        let mut view_mut = view;
        let data_ref = view_mut.try_borrow_mut()?;
        let (data, serialized_len) = Self::validate_and_load(view, &data_ref)?;
        // SAFETY: Same as load(). RefMut provides exclusive access and prevents
        // any other borrow on the same account.
        let guard: RefMut<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        Ok(Self {
            view,
            data,
            borrow: SerializedAccountBorrow::Mutable { guard },
            serialized_len,
            _serializer: PhantomData,
        })
    }

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

        // Drop any active borrow guard before the raw scrub below so the
        // subsequent `borrow_unchecked_mut` is unaliased. If the user
        // already called `release_borrow()` (e.g. a CPI between mutation
        // and close), this is a no-op.
        self.borrow = SerializedAccountBorrow::Released;

        // Defense-in-depth: write a closed-account sentinel ([u8::MAX; 8])
        // over the discriminator before pinocchio's close() zeros the
        // 48-byte header (lamports + data_len + owner). pinocchio's
        // close does not zero the data region — verified by the
        // `close_zeros_the_48_byte_header` test. If a future caller
        // restores data_len + owner without going through SVM zero-on-
        // allocate, the stale discriminator would otherwise allow a
        // reload with pre-close state. Mirrors `Slab::close`. Runs
        // unconditionally on borrow state: `release_borrow + close` is a
        // sanctioned call sequence (pre-CPI commit + derive-emitted
        // close), so the scrub must not be gated on holding a live
        // guard.
        //
        // SAFETY: `&mut self` plus the guard release above means no
        // other live borrow on this account; the view's data is valid
        // until pinocchio's `close()` reassigns ownership below.
        let data = unsafe { self_view.borrow_unchecked_mut() };
        if data.len() >= 8 {
            data[..8].copy_from_slice(&[u8::MAX; 8]);
        }

        self_view.close()?;
        Ok(())
    }

    fn exit(&mut self) -> pinocchio::ProgramResult {
        // Skip serialization if account was closed (lamports == 0, reassigned to system program).
        if self.view.lamports() == 0 {
            return Ok(());
        }
        // Belt-and-braces: the derive's `realloc` constraint does
        // release_borrow + reacquire after resize, but if someone resizes
        // through a non-derive path the guard's length would be stale.
        // Detect and fix before serializing.
        let stale = matches!(&self.borrow, SerializedAccountBorrow::Mutable { guard } if guard.len() != self.view.data_len());
        if stale {
            // Drop the stale guard directly rather than via release_borrow:
            // serializing through a stale-length guard would OOB on shrink.
            // The serialize below runs through the freshly reacquired guard.
            self.borrow = SerializedAccountBorrow::Released;
            self.reacquire_guard_only()?;
        }
        Self::serialize_mutable_borrow(
            &self.data,
            &mut self.borrow,
            &mut self.serialized_len,
        )?;
        Ok(())
    }
}

impl<T, S> crate::AccountRealloc for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
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
            self.release_borrow()?;
            crate::realloc_account(&mut view, new_space, &payer, zero)?;
            self.reacquire_guard_only()?;
        }
        Ok(())
    }
}

impl<T, S> Deref for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    type Target = T;
    fn deref(&self) -> &T {
        match &self.borrow {
            SerializedAccountBorrow::Mutable { .. } | SerializedAccountBorrow::Immutable { .. } => {
                &self.data
            }
            SerializedAccountBorrow::Released => panic!("account borrow released (closed)"),
        }
    }
}

impl<T, S> DerefMut for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    fn deref_mut(&mut self) -> &mut T {
        match &self.borrow {
            SerializedAccountBorrow::Mutable { .. } => &mut self.data,
            SerializedAccountBorrow::Immutable { .. } => {
                panic!("use #[account(mut)] for mutable access")
            }
            SerializedAccountBorrow::Released => panic!("account borrow released (closed)"),
        }
    }
}

impl<T, S> AsRef<AccountView> for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    fn as_ref(&self) -> &AccountView {
        &self.view
    }
}

impl<T, S> AsRef<Address> for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    fn as_ref(&self) -> &Address {
        self.view.address()
    }
}

impl<T, S> crate::ToCpiHandle for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    #[inline(always)]
    fn to_cpi_handle(&self) -> crate::CpiHandle<'_> {
        crate::AnchorAccount::cpi_handle(self)
    }
}

impl<T, S> crate::ToCpiHandleMut for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    #[inline(always)]
    fn try_to_cpi_handle_mut(
        &mut self,
    ) -> Result<crate::CpiHandleMut<'_>, solana_program_error::ProgramError> {
        crate::AnchorAccount::try_cpi_handle_mut(self)
    }
}

/// Forward `Discriminator` from a `SerializedAccount<T, S>` to its inner type.
/// Lets the `#[account(zeroed)]` derive codegen look up the disc via the field
/// type directly (`<BorshAccount<Counter> as Discriminator>::DISCRIMINATOR`).
impl<T, S> Discriminator for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    const DISCRIMINATOR: &'static [u8] = T::DISCRIMINATOR;
}

#[doc(hidden)]
impl<T, S> crate::IdlAccountType for SerializedAccount<T, S>
where
    T: Owner + Discriminator + crate::IdlAccountType,
    S: AnchorAccountSerialize<T>,
{
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = T::__IDL_ACCOUNT_ENTRY;
    const __IDL_TYPE_DEF: Option<&'static str> = T::__IDL_TYPE_DEF;
    fn __register_idl_deps(
        accounts: &mut ::alloc::vec::Vec<&'static str>,
        types: &mut ::alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}

/// Init for `SerializedAccount<T, S>`: creates the account, writes the
/// discriminator, then deserializes `T` via `S` from the zero-filled tail.
/// Types whose codec rejects all-zero encoding cannot be `init`-ed this way.
impl<T, S> AccountInitialize for SerializedAccount<T, S>
where
    T: Owner + Discriminator,
    S: AnchorAccountSerialize<T>,
{
    type Params<'a> = ();

    #[inline(always)]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        _owner: &Address,
        _params: &(),
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError> {
        let disc: &[u8; 8] = T::DISCRIMINATOR
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        match signer_seeds {
            Some(seeds) => crate::create_account_signed(payer, account, space, &T::OWNER, seeds)?,
            None => crate::create_account(payer, account, space, &T::OWNER)?,
        }
        let mut view_mut = *account;
        let data_ref = view_mut.try_borrow_mut()?;
        let mut guard: RefMut<'static, [u8]> = unsafe { core::mem::transmute(data_ref) };
        match guard.first_chunk_mut::<DISC_LEN>() {
            Some(dst) => *dst = *disc,
            None => return Err(ProgramError::AccountDataTooSmall),
        }
        let (data, serialized_len) = Self::deserialize_payload_with_len(&guard[DISC_LEN..])?;
        Ok(Self {
            view: *account,
            data,
            borrow: SerializedAccountBorrow::Mutable { guard },
            serialized_len,
            _serializer: PhantomData,
        })
    }
}
