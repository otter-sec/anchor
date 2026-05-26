use {
    crate::require,
    core::ops::Deref,
    pinocchio::{account::AccountView, address::Address, instruction::InstructionAccount},
    solana_program_error::{ProgramError, ProgramResult},
};

#[cfg(feature = "compat")]
use pinocchio::account::{Ref, RefMut};

/// Zero-cost CPI handle that borrows an anchor account at the Rust level.
///
/// Obtained via [`AnchorAccount::cpi_handle`] (shared borrow) or by erasing a
/// [`CpiHandleMut`] produced from [`AnchorAccount::cpi_handle_mut`].
/// Pinocchio's `borrow_state` is never modified — CPI is routed through
/// `invoke_signed_unchecked` by [`CpiContext::invoke`].
///
/// Deliberately does NOT implement `Deref<Target = AccountView>` to
/// prevent accidental use with pinocchio's checked invoke builders.
#[derive(Clone, Copy)]
pub struct CpiHandle<'a> {
    view: &'a AccountView,
    writable: bool,
}

/// Typed mutable CPI handle for API-facing CPI account structs.
///
/// This carries the exclusive-borrow provenance at construction time, then
/// erases into [`CpiHandle`] for invocation.
#[derive(Clone, Copy)]
pub struct CpiHandleMut<'a> {
    view: &'a AccountView,
}

impl<'a> CpiHandle<'a> {
    #[inline(always)]
    pub fn readonly(view: &'a AccountView) -> Self {
        Self {
            view,
            writable: false,
        }
    }

    #[inline(always)]
    pub fn writable(view: &'a mut AccountView) -> Self {
        Self {
            view,
            writable: true,
        }
    }

    /// The account's on-chain address.
    ///
    /// Returns a reference with the inner `'a` lifetime so callers can
    /// build `InstructionAccount<'a>` values without tying the result to
    /// the borrow of `&self`.
    #[inline(always)]
    pub fn address(&self) -> &'a Address {
        self.view.address()
    }

    /// Whether this handle was obtained via `cpi_handle_mut`.
    #[inline(always)]
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// Whether the underlying account is a signer on the transaction.
    #[inline(always)]
    pub fn is_signer(&self) -> bool {
        self.view.is_signer()
    }

    /// Access the underlying `AccountView` for CPI account construction.
    ///
    /// Restricted to the crate so external code cannot extract the view
    /// and pass it to pinocchio's checked invoke.
    #[inline(always)]
    pub(crate) fn account_view(&self) -> &'a AccountView {
        self.view
    }
}

impl<'a> CpiHandleMut<'a> {
    #[inline(always)]
    pub fn writable(view: &'a mut AccountView) -> Self {
        Self { view }
    }

    /// The account's on-chain address.
    #[inline(always)]
    pub fn address(&self) -> &'a Address {
        self.view.address()
    }

    /// Mutable handles always erase to writable CPI handles.
    #[inline(always)]
    pub fn is_writable(&self) -> bool {
        true
    }

    /// Whether the underlying account is a signer on the transaction.
    #[inline(always)]
    pub fn is_signer(&self) -> bool {
        self.view.is_signer()
    }
}

impl<'a> From<CpiHandleMut<'a>> for CpiHandle<'a> {
    #[inline(always)]
    fn from(handle: CpiHandleMut<'a>) -> Self {
        Self {
            view: handle.view,
            writable: true,
        }
    }
}

/// Converts a CPI accounts struct into instruction metadata and handles.
///
/// Implemented by generated CPI accounts structs. Each field maps to an
/// [`InstructionAccount`] (address + writable/signer flags) and an erased
/// [`CpiHandle`] for the actual invocation.
pub trait ToCpiAccounts<'a> {
    /// Produce instruction account metadata for the CPI instruction.
    fn to_instruction_accounts(&self) -> alloc::vec::Vec<InstructionAccount<'a>>;

    /// Collect all CPI handles for the invocation.
    fn to_cpi_handles(&self) -> alloc::vec::Vec<CpiHandle<'a>>;
}

pub trait AnchorAccount: Deref<Target = Self::Data> + Sized {
    type Data;

    /// Whether this account wrapper requires the transaction account meta to
    /// be marked as a signer in generated clients and CPI account structs.
    const IS_SIGNER: bool = false;

    /// Minimum account data length for this type. When > 0, PDA
    /// verification can skip `sol_curve_validate_point`: a non-empty
    /// account was created via CreateAccount/Allocate (which requires
    /// signing), and `invoke_signed` already includes the curve check.
    ///
    /// Non-empty wrappers override this with their schema-specific minimum.
    /// UncheckedAccount / zero-data wrappers leave it at `0` (forces the curve
    /// check).
    const MIN_DATA_LEN: usize = 0;

    fn load(view: AccountView) -> core::result::Result<Self, ProgramError>;

    /// Load an account for mutable access.
    ///
    /// # Safety
    ///
    /// No other live `&mut` to the same account data may exist while the
    /// returned value is alive. In derive-generated code the bitvec
    /// duplicate-account check enforces this; direct callers must uphold
    /// it themselves.
    ///
    /// Default impl validates `is_writable` and delegates to `load()`.
    /// Data-carrying wrappers (`Account<T>`, `BorshAccount<T>`, `Slab<H, T>`)
    /// override to use `borrow_unchecked_mut` for write provenance.
    /// `Signer` overrides with a fused `is_signer` + `is_writable` check.
    #[inline(always)]
    unsafe fn load_mut(view: AccountView) -> core::result::Result<Self, ProgramError> {
        if !view.is_writable() {
            return Err(crate::ErrorCode::ConstraintMut.into());
        }
        Self::load(view)
    }

    /// Like [`load_mut`], but called right after
    /// `AccountInitialize::create_and_initialize`. Owner, discriminator,
    /// and min-length checks are tautologies on this path, so data-carrying
    /// wrappers override to skip them. Default forwards to [`load_mut`].
    ///
    /// # Safety
    ///
    /// Same as [`load_mut`]: no other live `&mut` to the same account data.
    ///
    /// [`load_mut`]: Self::load_mut
    #[inline(always)]
    unsafe fn load_mut_after_init(view: AccountView) -> core::result::Result<Self, ProgramError> {
        Self::load_mut(view)
    }

    fn account(&self) -> &AccountView;

    fn exit(&mut self) -> ProgramResult {
        Ok(())
    }

    /// v1-compatible alias for the account address.
    #[cfg(feature = "compat")]
    #[inline(always)]
    fn key(&self) -> crate::solana_program::pubkey::Pubkey {
        *self.account().address()
    }

    fn close(&mut self, mut destination: AccountView) -> ProgramResult {
        let mut self_view = *self.account();
        let dest_lamports = destination
            .lamports()
            .checked_add(self_view.lamports())
            .ok_or(ProgramError::ArithmeticOverflow)?;
        destination.set_lamports(dest_lamports);
        self_view.set_lamports(0);
        self_view.close()?;
        Ok(())
    }

    /// Obtain a read-only CPI handle for this account.
    ///
    /// The handle borrows `self`, preventing mutable typed access while
    /// it is alive. The handle's `is_writable` flag is `false`.
    #[inline(always)]
    fn cpi_handle(&self) -> CpiHandle<'_> {
        CpiHandle {
            view: self.account(),
            writable: false,
        }
    }

    /// Obtain a writable CPI handle for this account.
    ///
    /// The handle borrows `self` mutably, preventing any typed access
    /// while it is alive.
    ///
    /// # Panics
    ///
    /// Panics if the underlying account is not marked writable in
    /// the transaction.
    #[inline(always)]
    fn cpi_handle_mut(&mut self) -> CpiHandleMut<'_> {
        self.try_cpi_handle_mut()
            .expect("cpi_handle_mut called on a read-only account")
    }

    /// Fallible variant of [`cpi_handle_mut`](Self::cpi_handle_mut).
    ///
    /// Returns [`ProgramError::InvalidArgument`] when the underlying account
    /// is not marked writable in the transaction.
    #[inline(always)]
    fn try_cpi_handle_mut(&mut self) -> Result<CpiHandleMut<'_>, ProgramError> {
        require!(self.account().is_writable(), ProgramError::InvalidArgument);
        Ok(CpiHandleMut {
            view: self.account(),
        })
    }
}

/// Account wrapper capability for `#[account(realloc = ...)]`.
///
/// The derive emits a call to this trait instead of deciding realloc safety
/// from syntactic type names. That lets rustc resolve aliases and wrapper
/// forwards normally: unsupported wrappers simply do not implement the trait.
pub trait AccountRealloc: AnchorAccount {
    fn realloc_account(
        &mut self,
        new_space: usize,
        payer: AccountView,
        zero: bool,
    ) -> ProgramResult;
}

/// Account-like value that can be passed into a CPI account struct.
///
/// This is the v2 equivalent of v1's `ToAccountInfo` for CPI construction:
/// callers get a [`CpiHandle`] instead of cloning an `AccountInfo`.
pub trait ToCpiHandle {
    fn to_cpi_handle(&self) -> CpiHandle<'_>;
}

/// Account-like value that can be passed into a writable CPI account slot.
pub trait ToCpiHandleMut {
    fn try_to_cpi_handle_mut(&mut self) -> Result<CpiHandleMut<'_>, ProgramError>;

    #[inline(always)]
    fn to_cpi_handle_mut(&mut self) -> CpiHandleMut<'_> {
        self.try_to_cpi_handle_mut()
            .expect("to_cpi_handle_mut called on a read-only account")
    }
}

impl<T: ToCpiHandle + ?Sized> ToCpiHandle for &T {
    #[inline(always)]
    fn to_cpi_handle(&self) -> CpiHandle<'_> {
        (*self).to_cpi_handle()
    }
}

impl<T: ToCpiHandle + ?Sized> ToCpiHandle for &mut T {
    #[inline(always)]
    fn to_cpi_handle(&self) -> CpiHandle<'_> {
        (**self).to_cpi_handle()
    }
}

impl<T: ToCpiHandleMut + ?Sized> ToCpiHandleMut for &mut T {
    #[inline(always)]
    fn try_to_cpi_handle_mut(&mut self) -> Result<CpiHandleMut<'_>, ProgramError> {
        (**self).try_to_cpi_handle_mut()
    }
}

impl ToCpiHandle for CpiHandle<'_> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> CpiHandle<'_> {
        *self
    }
}

impl ToCpiHandle for CpiHandleMut<'_> {
    #[inline(always)]
    fn to_cpi_handle(&self) -> CpiHandle<'_> {
        (*self).into()
    }
}

impl ToCpiHandleMut for CpiHandleMut<'_> {
    #[inline(always)]
    fn try_to_cpi_handle_mut(&mut self) -> Result<CpiHandleMut<'_>, ProgramError> {
        Ok(*self)
    }
}

impl ToCpiHandle for AccountView {
    #[inline(always)]
    fn to_cpi_handle(&self) -> CpiHandle<'_> {
        CpiHandle::readonly(self)
    }
}

impl ToCpiHandleMut for AccountView {
    #[inline(always)]
    fn try_to_cpi_handle_mut(&mut self) -> Result<CpiHandleMut<'_>, ProgramError> {
        require!(self.is_writable(), ProgramError::InvalidArgument);
        Ok(CpiHandleMut::writable(self))
    }
}

/// Account-like value that can provide its on-chain address.
///
/// This is intentionally implemented blanketly for every [`AnchorAccount`]:
/// `AnchorAccount::account()` already exposes the underlying `AccountView`,
/// so boxed and typed account wrappers can all be used uniformly as address
/// field references in generated constraint code.
pub trait AccountAddress {
    fn account_address(&self) -> &Address;
}

impl<T: AnchorAccount> AccountAddress for T {
    #[inline(always)]
    fn account_address(&self) -> &Address {
        self.account().address()
    }
}

impl<T: AccountAddress> AccountAddress for Option<T> {
    #[inline(always)]
    fn account_address(&self) -> &Address {
        self.as_ref()
            .expect("optional account is None")
            .account_address()
    }
}

/// v1-compatible utility methods for raw remaining-account views.
#[cfg(feature = "compat")]
pub trait AccountViewCompat {
    fn key(&self) -> crate::solana_program::pubkey::Pubkey;

    fn data_is_empty(&self) -> bool;

    fn try_data_len(&self) -> Result<usize, ProgramError>;

    fn try_borrow_data(&self) -> Result<Ref<'_, [u8]>, ProgramError>;

    fn try_borrow_mut_data(&mut self) -> Result<RefMut<'_, [u8]>, ProgramError>;
}

#[cfg(feature = "compat")]
impl AccountViewCompat for AccountView {
    #[inline(always)]
    fn key(&self) -> crate::solana_program::pubkey::Pubkey {
        *self.address()
    }

    #[inline(always)]
    fn data_is_empty(&self) -> bool {
        self.data_len() == 0
    }

    #[inline(always)]
    fn try_data_len(&self) -> Result<usize, ProgramError> {
        Ok(self.data_len())
    }

    #[inline(always)]
    fn try_borrow_data(&self) -> Result<Ref<'_, [u8]>, ProgramError> {
        self.try_borrow()
    }

    #[inline(always)]
    fn try_borrow_mut_data(&mut self) -> Result<RefMut<'_, [u8]>, ProgramError> {
        self.try_borrow_mut()
    }
}

/// Lamports related utility methods for accounts.
pub trait Lamports: AsRef<AccountView> {
    /// Get the lamports of the account.
    #[inline(always)]
    fn get_lamports(&self) -> u64 {
        self.as_ref().lamports()
    }

    /// Add lamports to the account.
    ///
    /// This method is useful for transferring lamports from a PDA.
    ///
    /// # Requirements
    ///
    /// 1. The account must be marked `mut`.
    /// 2. The total lamports before the transaction must equal the total
    ///    lamports after the transaction.
    ///
    /// See [`Lamports::sub_lamports`] for subtracting lamports.
    #[inline(always)]
    fn add_lamports(&self, amount: u64) -> Result<&Self, ProgramError> {
        let mut view = *self.as_ref();
        view.set_lamports(
            self.get_lamports()
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?,
        );
        Ok(self)
    }

    /// Subtract lamports from the account.
    ///
    /// This method is useful for transferring lamports from a PDA.
    ///
    /// # Requirements
    ///
    /// 1. The account must be owned by the executing program.
    /// 2. The account must be marked `mut`.
    /// 3. The total lamports before the transaction must equal the total
    ///    lamports after the transaction.
    ///
    /// See [`Lamports::add_lamports`] for adding lamports.
    #[inline(always)]
    fn sub_lamports(&self, amount: u64) -> Result<&Self, ProgramError> {
        let mut view = *self.as_ref();
        view.set_lamports(
            self.get_lamports()
                .checked_sub(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?,
        );
        Ok(self)
    }
}

impl<T: AsRef<AccountView>> Lamports for T {}

/// Declares which program owns accounts of this data type.
///
/// For your own program's types, `#[account]` generates this automatically
/// from the program's declared ID.
///
/// External crates implement this with their program's address:
/// ```ignore
/// impl Owner for TokenAccountData {
///     const OWNER: Address = Token::ID;
/// }
/// ```
pub trait Owner {
    const OWNER: Address;
}

/// Declares the on-chain address for a program marker type.
///
/// `Address` is re-exported from `pinocchio`, which itself re-exports
/// `solana_address::Address`. That means built-in markers such as
/// `Token::id()` and `System::id()` can be passed directly to modern Solana
/// instruction APIs that use `Address` or compatibility aliases named
/// `Pubkey`.
pub trait Id {
    fn id() -> Address;
    /// Well-known base58 program address for IDL emission. Empty string
    /// signals "no address to advertise in the IDL" — consumed by
    /// `IdlAccountType::__IDL_ADDRESS` on `Program<T>` and converted to
    /// `None` there.
    const IDL_ADDRESS: &'static str = "";
}

/// Declares multiple valid on-chain addresses for an interface program marker.
pub trait Ids {
    fn ids() -> &'static [Address];
}

pub trait Discriminator {
    const DISCRIMINATOR: &'static [u8];
}

/// Client-side account deserialization. Mirrors v1 anchor-lang's trait so
/// `anchor-client` can fetch raw account bytes and decode them into the
/// user's `#[account]` struct. The `#[account]` macro emits two impl
/// bodies:
///
///   - Borsh mode (`#[account(borsh)]`): check disc, run `BorshDeserialize`.
///   - Pod mode (default): check disc, `bytemuck::pod_read_unaligned` on
///     the post-disc bytes.
///
/// Not used by the on-chain account wrappers (`BorshAccount` / `Slab`),
/// which read directly from `AccountView` borrows; this is purely the
/// off-chain client helper.
pub trait AccountDeserialize: Sized {
    /// Verify the leading discriminator and decode. Generated impls perform
    /// the discriminator check; the default forwards to
    /// `try_deserialize_unchecked`.
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self, ProgramError> {
        Self::try_deserialize_unchecked(buf)
    }

    /// Decode without verifying the discriminator. Used during initialization
    /// when the bytes are zero or otherwise not yet stamped with the disc.
    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self, ProgramError>;
}

/// Wrapper-level init: creates the on-chain account and returns a loaded
/// `Self`. `Slab<H, T>` and `BorshAccount<T>` get this automatically;
/// custom wrappers implement it directly.
pub trait AccountInitialize: Sized {
    type Params<'a>: Default;

    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        owner: &Address,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<Self, ProgramError>;
}

/// Marker for account wrappers that may be allocated with an explicit
/// foreign owner through `#[account(init, owner = ...)]`.
///
/// Typed account wrappers intentionally do not implement this: their init
/// paths stamp and load Anchor-owned data, so they must stay owned by the
/// current program. `UncheckedAccount` is the escape hatch for allocating
/// bytes that a foreign program will initialize or validate later.
pub trait ForeignOwnerInit: AccountInitialize {}

// ---------------------------------------------------------------------------
// Extensible constraint system
// ---------------------------------------------------------------------------

/// Trait implemented by each constraint marker type for every account
/// type it applies to. Each method defaults to `Ok(())`, so CHECK-only
/// constraints only need to override `check`, INIT-only constraints
/// only override `init`, etc.
///
/// # Lifecycle mapping
///
/// | `#[account(...)]` spelling                         | Methods called        |
/// |----------------------------------------------------|-----------------------|
/// | `ns::key = v` (non-init field)                     | `check`               |
/// | `init, ns::key = v`                                | `init`                |
/// | `init_if_needed, ns::key = v` (creating)           | `init`, then `check`  |
/// | `init_if_needed, ns::key = v` (already exists)     | `check`               |
/// | `update(ns::key = v)`                              | `update`              |
/// | Any of the above                                    | `exit` (exit phase)  |
///
/// There is deliberately **no blanket `impl<T: AccountConstraint<A>>
/// AccountConstraint<Option<A>> for T`** mirroring the `Box<T>` forwarder
/// in `accounts/boxed.rs`. Constraint calls on `Option<Field>` are emitted
/// by the derive inline as `if let Some(ref inner) = self.maybe_x { …
/// inline call … }` — they never dispatch through a blanket impl.
///
/// # Extending with third-party constraints
///
/// Any crate can define new constraint markers and implement
/// `AccountConstraint<SomeAccount>` for them. The derive routes
/// `ns::key = v`, `init`/`init_if_needed`-paired constraints, and the
/// `update(...)` wrapper through the appropriate method.
///
/// ```ignore
/// pub mod my_ns {
///     use anchor_lang_v2::AccountConstraint;
///     use pinocchio::program_error::ProgramError;
///
///     pub struct MinBalanceConstraint;
///
///     impl AccountConstraint<MyAccount> for MinBalanceConstraint {
///         type Value = u64;
///         fn check(account: &MyAccount, min: &u64) -> Result<(), ProgramError> {
///             if account.account().lamports() < *min {
///                 return Err(ProgramError::InsufficientFunds);
///             }
///             Ok(())
///         }
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct MyInstruction {
///     #[account(mut, my_ns::min_balance = 1_000_000)]
///     pub data: MyAccount,
/// }
/// ```
pub trait AccountConstraint<A> {
    /// The expected value type for this constraint. This is the type of
    /// the RHS expression in `#[account(namespace::key = <expr>)]`.
    ///
    /// Common choices:
    /// - `Address` for address comparisons (default for most constraints)
    /// - `AccountView` for constraints that need the full account view
    /// - `u8` / `u64` for numeric constraints
    type Value;

    /// Creation hook. Invoked on `init` and on the create branch of
    /// `init_if_needed` — whenever the account is being freshly
    /// produced by this instruction — after `AccountInitialize::
    /// create_and_initialize` has run. Mutable access so the
    /// constraint can stamp additional state.
    #[inline(always)]
    fn init(_account: &mut A, _value: &Self::Value) -> core::result::Result<(), ProgramError> {
        Ok(())
    }

    /// Runtime validation. Invoked on non-init fields and on the
    /// already-exists branch of `init_if_needed`. Read-only.
    #[inline(always)]
    fn check(_account: &A, _value: &Self::Value) -> core::result::Result<(), ProgramError> {
        Ok(())
    }

    /// Mutating hook. Invoked only when the constraint is written
    /// inside an `update(...)` wrapper, e.g.
    /// `#[account(update(my_ns::field = value))]`. Intended for
    /// constraints that set / rewrite on-chain state rather than
    /// validating it.
    #[inline(always)]
    fn update(_account: &mut A, _value: &Self::Value) -> core::result::Result<(), ProgramError> {
        Ok(())
    }

    /// Exit hook. Called during `AccountsExit::exit_accounts()` for
    /// every constraint attached to the field, regardless of how the
    /// field was introduced. Use for state that must be flushed on a
    /// successful instruction.
    #[inline(always)]
    fn exit(_account: &mut A, _value: &Self::Value) -> core::result::Result<(), ProgramError> {
        Ok(())
    }
}

pub struct Nested<T>(pub T);

impl<T> Deref for Nested<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Nested<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[doc(hidden)]
impl<T: crate::IdlAccountType> crate::IdlAccountType for Nested<T> {
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = T::__IDL_ACCOUNT_ENTRY;
    const __IDL_TYPE_DEF: Option<&'static str> = T::__IDL_TYPE_DEF;
    fn __register_idl_deps(
        accounts: &mut ::alloc::vec::Vec<&'static str>,
        types: &mut ::alloc::vec::Vec<&'static str>,
    ) {
        T::__register_idl_deps(accounts, types);
    }
}
