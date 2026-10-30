//! Anchor v2: Trait-based account system for Solana.
//!
//! `#![no_std]` compatible.

#![cfg_attr(any(target_os = "solana", target_arch = "bpf"), no_std)]

extern crate alloc;
extern crate self as anchor_lang;

/// Hidden compatibility re-export used by generated V2 host and IDL code.
#[doc(hidden)]
pub extern crate alloc as __alloc;

pub mod accounts;
mod account_meta;
mod buffers;
pub mod context;
mod context_cpi;
pub mod cpi;
pub mod cursor;
mod dispatch;
pub mod event;
mod fixed_vec;
pub mod hash;
mod idl;
mod instruction;
#[doc(hidden)]
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub mod idl_build;
pub mod loader;
pub mod pod;
pub mod prelude;
mod program;
pub mod programs;
pub mod system_program;
#[cfg(feature = "testing")]
pub mod testing;
mod traits;

// Re-export derive macros and bytemuck for generated code
pub use cpi::realloc_account;
pub use account_meta::AccountMeta;
#[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
pub use account_meta::ToAccountMetas;
/// Chunked 4×u64 equality compare for `Address`. Preferred over `==`
/// on `&Address`. See <https://github.com/anza-xyz/solana-sdk/issues/345>.
pub use pinocchio::address::address_eq;
#[cfg(feature = "compat")]
pub use solana_address::address as pubkey;
/// Re-export address helpers from solana-address.
pub use solana_address::{address, declare_id};
/// Implementation detail of [`solana_msg`] - re-exported for macro access only.
#[doc(hidden)]
pub use solana_msg;
#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {{
        $crate::solana_msg::msg!($($arg)*);
    }};
}

/// v1-compat logger. Gated behind `compat`.
///
/// # Example
///
/// ```ignore
/// debug!("raw bytes: {:?}", &data[..32]);
/// ```
#[cfg(feature = "compat")]
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {{ $crate::msg!($($arg)*) }};
}
// Re-export wincode for instruction data serialization
pub use wincode;

pub use buffers::*;
pub use fixed_vec::FixedVec;
pub use idl::{__idl, IdlAccountType, IdlEntries, MAX_IDL_SEQUENCE};

#[doc(hidden)]
#[macro_export]
macro_rules! __anchor_program_allocator {
    () => {
        $crate::pinocchio::default_allocator!();
    };
}

/// Borsh-compatible wincode config: u8 enum tags + fixed u32 LE length
/// prefixes. Used for borsh-compatible v2 serialization paths (instruction
/// args, default events, `BorshAccount<T>`) so the on-chain wire format matches
/// borsh exactly, while keeping wincode's faster encoding path.
///
/// # ⚠ Incompatibilities with the borsh crate
///
/// Wincode with this config is byte-identical to borsh for the shapes Anchor
/// programs commonly use (integers, fixed arrays, `Vec`, `String`, `Option`,
/// tagged enums, nested structs). The following shapes are supported but are
/// NOT byte-identical — if a program built on Anchor v1 (real borsh) used
/// them, the on-chain bytes will NOT necessarily round-trip cleanly through v2:
///
/// - **`HashMap` / `HashSet`**: borsh sorts entries by key, wincode preserves
///   insertion order. Use `BTreeMap` / `BTreeSet` or `Vec<(K, V)>` if you
///   need canonical ordering.
/// - **`f32` / `f64`**: wincode accepts NaN while borsh rejects it. Anchor v2
///   intentionally follows wincode here, so floating-point values (including
///   NaN) are supported. Programs migrating from v1 should account for this
///   serialization difference when validating or comparing float values.
/// - **Dynamic collections of zero-sized elements** (`Vec<()>`, and the same
///   shape for any other zero-sized `T`): borsh rejects them. Wincode writes a
///   `u32` length prefix and no element bytes, then reads that length back.
///   This does not show up in ordinary account data. Wincode still bounds the
///   length through its preallocation limit.
///
/// Programs that don't use these types are unaffected.
///
/// # Borrowed zero-copy caveat
///
/// Borsh's u32 length prefix means borrowed wide slices like `&[u64]` start
/// four bytes into the buffer and therefore fail wincode's zero-copy
/// alignment guard. `BORSH_CONFIG` keeps that guard enabled so safe
/// deserialization cannot form a misaligned Rust reference; use owned
/// arguments (`Vec<u64>`) or align-1 borrowed data (`&[u8]`, `&str`) instead.
pub const BORSH_CONFIG: BorshConfig = wincode::config::Configuration::new();

/// Solana allows at most 16 seeds when deriving a PDA.
pub const MAX_PAYER_SEEDS: usize = 16;
/// PDA payers append the canonical bump as a final signer seed.
pub const MAX_PAYER_SEEDS_WITH_BUMP: usize = MAX_PAYER_SEEDS + 1;

/// Concrete type of [`BORSH_CONFIG`]. Spelled out so a raw wincode bound can
/// name it (e.g. `T: wincode::SchemaRead<'de, BorshConfig>`). Most programs
/// should derive [`AnchorDeserialize`] / [`AnchorSerialize`] and bound on
/// those traits instead.
pub type BorshConfig = wincode::config::Configuration<
    true,
    { wincode::config::DEFAULT_PREALLOCATION_SIZE_LIMIT },
    wincode::len::FixIntLen<u32>,
    wincode::int_encoding::LittleEndian,
    wincode::int_encoding::FixInt,
    u8,
>;

/// Types Anchor can decode from borsh-shaped bytes with [`BORSH_CONFIG`].
///
/// This is the trait behind `#[derive(AnchorDeserialize)]`. The derive macro
/// and the trait share one name on purpose, like `borsh::BorshDeserialize`:
/// write `#[derive(AnchorDeserialize)]` on the type and `T: AnchorDeserialize`
/// in a bound.
///
/// `#[event]` derives it, so a client can decode an event with
/// `fn decode<T: Event + AnchorDeserialize>(..)` and never mention wincode.
pub trait AnchorDeserialize: for<'de> wincode::SchemaRead<'de, BorshConfig, Dst = Self> {}

impl<T> AnchorDeserialize for T where T: for<'de> wincode::SchemaRead<'de, BorshConfig, Dst = T> {}

/// Types Anchor can encode to borsh-shaped bytes with [`BORSH_CONFIG`].
///
/// Trait twin of `#[derive(AnchorSerialize)]`, with the same rules as
/// [`AnchorDeserialize`]: derive it, bound on it, never implement it by hand.
pub trait AnchorSerialize: wincode::SchemaWrite<BorshConfig, Src = Self> {}

impl<T> AnchorSerialize for T where T: wincode::SchemaWrite<BorshConfig, Src = T> {}

/// `#[derive(IdlType)]` — register a plain struct in the IDL's `types[]`
/// array. Add `#[idl(bytemuck)]` to an explicitly `#[repr(C)]`, Pod-safe
/// nested type when it is stored in a zero-copy account or bytemuck event.
///
/// **Opaque / unstable.** Apply this derive on user types you want to
/// surface in the generated IDL; do not call any of the emitted associated
/// items directly — they are implementation details of the `anchor idl
/// build` pipeline and will change without notice. The emitted impl body
/// is gated on the **end-user crate's** local `idl-build` feature, so
/// non-IDL builds pay nothing.
pub use anchor_derive_accounts::IdlType;
// ---------------------------------------------------------------------------
// Client-side types — for building instructions off-chain (tests, CPI, SDK)
// ---------------------------------------------------------------------------
pub use anchor_derive_accounts::{emit_cpi, event_cpi};
#[doc(hidden)]
pub use cpi::create_account_with_signers;
/// **Opaque / unstable.** Re-exported so derive-emitted code in user
/// crates can name the trait. Do not implement this trait by hand or call
/// its associated items — they are implementation details of the IDL
/// build pipeline and will change without notice. See [`idl_build`] for
/// the trait definition.
/// Metadata for a single account in a transaction instruction.
///
/// This is a fixed-size, allocator-neutral value used by the public V2 account
/// builder traits. Host-side instruction construction converts it to the SDK's
/// owned `solana_instruction::AccountMeta` representation at the facade
/// boundary.
pub use {
    accounts::{AccountInitialize, SlabInit},
    anchor_derive_accounts::{
        __erase, access_control, account, constant, declare_program, emit, error_code, event,
        pod_wrapper, program, Accounts, AnchorDeserialize, AnchorSerialize, InitSpace,
        ToCpiAccounts,
    },
    bytemuck,
    context::{Bumps, Context},
    context_cpi::{unchecked_invoke_signed_fixed, CpiContext},
    cpi::{
        create_account, create_account_signed, create_program_address,
        find_and_verify_program_address, find_and_verify_program_address_skip_curve,
        find_program_address, verify_program_address,
    },
    cursor::AccountCursor,
    dispatch::{run_handler, TryAccounts},
    event::{sol_log_data, Event},
    hash::sha256,
    instruction::CpiInstruction,
    loader::AccountLoader,
    pinocchio::{self, account::AccountView, address::Address},
    traits::*,
    wincode::{SchemaRead, SchemaWrite},
};

/// Re-export of the Solana SDK `Instruction` + `AccountMeta` types under a v1-
/// compatible module path. Lets users write
/// `use anchor_lang::solana_program::instruction::{Instruction, AccountMeta}`
/// without adding `solana-instruction` to their `Cargo.toml`.
pub mod solana_program {
    pub mod clock {
        pub use pinocchio::sysvars::clock::*;
    }

    pub mod hash {
        pub use solana_sha256_hasher::*;
    }

    pub mod instruction {
        #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
        pub use solana_instruction::{AccountMeta, Instruction};

        #[cfg(any(target_os = "solana", target_arch = "bpf"))]
        pub use crate::AccountMeta;
    }

    #[cfg(feature = "compat")]
    pub use crate::pubkey;
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    pub use solana_system_interface::instruction as system_instruction;

    #[cfg(feature = "compat")]
    pub mod pubkey {
        pub type Pubkey = crate::Address;
    }

    pub mod rent {
        pub use solana_sysvar::rent::*;
    }

    pub mod sysvar {
        pub use pinocchio::sysvars::Sysvar;
    }

    pub mod program {
        pub use crate::program::*;
    }
}

/// Serializes instruction data: discriminator prefix + LE-encoded args.
pub trait InstructionData: Discriminator {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    fn try_data(&self) -> core::result::Result<InstructionDataBuffer, solana_program_error::ProgramError>;

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    fn data(&self) -> InstructionDataBuffer;

    /// Fallible instruction serialization for allocator-free runtime paths.
    ///
    /// Existing implementations that only provide `data()` remain source
    /// compatible. Generated V2 instruction types override this method so a
    /// full buffer reports `InvalidInstructionData` instead of panicking.
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    fn try_data(&self) -> core::result::Result<InstructionDataBuffer, solana_program_error::ProgramError> {
        Ok(self.data())
    }
}

/// Compile-time account-size calculation. Derived via `#[derive(InitSpace)]`.
/// Typically used to size account rent: `space = 8 + MyAccount::INIT_SPACE`.
///
/// The derive handles Borsh-size accounting for variable-length fields via a
/// `#[max_len(N)]` helper attribute on `String` / `Vec<T>` fields. POD accounts
/// that use the default wincode backing should just use `core::mem::size_of`.
pub trait Space {
    const INIT_SPACE: usize;
}

macro_rules! impl_space_for_primitives {
    ($($ty:ty => $space:expr),* $(,)?) => {
        $(
            impl Space for $ty {
                const INIT_SPACE: usize = $space;
            }
        )*
    };
}

impl_space_for_primitives! {
    bool => 1,
    i8 => 1,
    u8 => 1,
    i16 => 2,
    u16 => 2,
    i32 => 4,
    u32 => 4,
    f32 => 4,
    i64 => 8,
    u64 => 8,
    f64 => 8,
    i128 => 16,
    u128 => 16,
    Address => 32,
}

impl<T: Space, const N: usize> Space for [T; N] {
    const INIT_SPACE: usize = T::INIT_SPACE * N;
}

impl<T: Space, const N: usize> Space for FixedVec<T, N> {
    const INIT_SPACE: usize = 4 + T::INIT_SPACE * N;
}

#[doc(hidden)]
pub mod __private {
    use crate::CpiHandle;
    use pinocchio::account::AccountView;

    /// Used by `#[derive(InitSpace)]` on enums to pick the largest variant size.
    pub const fn max(a: usize, b: usize) -> usize {
        [a, b][(a < b) as usize]
    }

    #[inline(always)]
    pub fn readonly_cpi_handle_for_account_field<'a>(
        view: &'a AccountView,
        needs_relax: bool,
    ) -> CpiHandle<'a> {
        if needs_relax {
            CpiHandle::readonly_with_flags(view, false, true)
        } else {
            CpiHandle::readonly(view)
        }
    }
}

/// Result type.
pub type Result<T> = core::result::Result<T, solana_program_error::ProgramError>;

/// Error type — just ProgramError for no_std.
pub type Error = solana_program_error::ProgramError;

/// Error codes matching Anchor v1's ErrorCode variants.
/// Used by constraint codegen.
pub enum ErrorCode {
    AccountNotEnoughKeys,
    ConstraintMut,
    ConstraintSigner,
    ConstraintSeeds,
    ConstraintHasOne,
    ConstraintAddress,
    ConstraintAccountIsNone,
    ConstraintClose,
    ConstraintOwner,
    ConstraintSpace,
    ConstraintRentExempt,
    ConstraintRaw,
    ConstraintExecutable,
    ConstraintZero,
    InstructionDidNotDeserialize,
    DeclaredProgramIdMismatch,
    InstructionFallbackNotFound,
    RequireViolated,
    RequireEqViolated,
    RequireNeqViolated,
    RequireKeysEqViolated,
    RequireKeysNeqViolated,
    RequireGtViolated,
    RequireGteViolated,
}

impl From<ErrorCode> for solana_program_error::ProgramError {
    #[cold]
    fn from(e: ErrorCode) -> Self {
        match e {
            ErrorCode::AccountNotEnoughKeys => {
                solana_program_error::ProgramError::NotEnoughAccountKeys
            }
            // Custom codes are aligned with Anchor v1's numbering
            ErrorCode::ConstraintMut => solana_program_error::ProgramError::Custom(2000),
            ErrorCode::ConstraintSigner => {
                solana_program_error::ProgramError::MissingRequiredSignature
            }
            ErrorCode::ConstraintSeeds => solana_program_error::ProgramError::InvalidSeeds,
            ErrorCode::ConstraintHasOne => solana_program_error::ProgramError::Custom(2001),
            ErrorCode::ConstraintAddress => solana_program_error::ProgramError::Custom(2012),
            ErrorCode::ConstraintAccountIsNone => solana_program_error::ProgramError::Custom(2020),
            ErrorCode::ConstraintClose => solana_program_error::ProgramError::Custom(2011),
            ErrorCode::ConstraintOwner => solana_program_error::ProgramError::IllegalOwner,
            ErrorCode::ConstraintSpace => solana_program_error::ProgramError::Custom(2019),
            ErrorCode::ConstraintRentExempt => solana_program_error::ProgramError::Custom(2005),
            ErrorCode::ConstraintRaw => solana_program_error::ProgramError::Custom(2003),
            ErrorCode::ConstraintExecutable => solana_program_error::ProgramError::Custom(2007),
            ErrorCode::ConstraintZero => solana_program_error::ProgramError::Custom(2013),
            ErrorCode::InstructionDidNotDeserialize => {
                solana_program_error::ProgramError::InvalidInstructionData
            }
            ErrorCode::DeclaredProgramIdMismatch => {
                solana_program_error::ProgramError::IncorrectProgramId
            }
            ErrorCode::InstructionFallbackNotFound => {
                solana_program_error::ProgramError::InvalidInstructionData
            }
            ErrorCode::RequireViolated => solana_program_error::ProgramError::Custom(2500),
            ErrorCode::RequireEqViolated => solana_program_error::ProgramError::Custom(2501),
            ErrorCode::RequireKeysEqViolated => solana_program_error::ProgramError::Custom(2502),
            ErrorCode::RequireNeqViolated => solana_program_error::ProgramError::Custom(2503),
            ErrorCode::RequireKeysNeqViolated => solana_program_error::ProgramError::Custom(2504),
            ErrorCode::RequireGtViolated => solana_program_error::ProgramError::Custom(2505),
            ErrorCode::RequireGteViolated => solana_program_error::ProgramError::Custom(2506),
        }
    }
}

/// Guardrail: verify that the runtime-supplied `program_id` matches this
/// program's `declare_id!()`. Gated behind the `guardrails` feature —
/// when disabled, compiles away entirely.
#[inline(always)]
pub fn check_program_id(
    _program_id: &Address,
    _declared: &Address,
) -> core::result::Result<(), solana_program_error::ProgramError> {
    #[cfg(feature = "guardrails")]
    if _program_id != _declared {
        return Err(ErrorCode::DeclaredProgramIdMismatch.into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// require! macros — no_std compatible
// ---------------------------------------------------------------------------

/// Creates a [`ProgramError`](crate::Error) from an Anchor-style error code.
///
/// This is a v1 syntax compatibility shim. Unlike v1's `error!` proc macro,
/// it does not allocate a rich `AnchorError` with source, name, message, or
/// compared-value metadata; v2 keeps runtime errors as `ProgramError` and
/// routes error text through the IDL.
#[cfg(feature = "compat")]
#[macro_export]
macro_rules! error {
    ($error:expr $(,)?) => {{
        ::core::convert::Into::<$crate::Error>::into($error)
    }};
}

/// Returns `Err(error!(...))`.
///
/// This is the expression-only v1 compatibility form for v2's `ProgramError`
/// error model.
#[cfg(feature = "compat")]
#[macro_export]
macro_rules! err {
    ($error:expr $(,)?) => {{
        Err($crate::error!($error))
    }};
}

/// Ensures a condition is true, otherwise returns an error.
///
/// Can be used with or without a custom error code.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(amount: u64) -> Result<()> {
///     require!(amount > 0, ConstraintRaw);
///     require!(amount > 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require {
    ($invariant:expr, $error:tt $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if !($invariant) {
            return Err($crate::ErrorCode::$error.into());
        }
    }};
    ($invariant:expr, $error:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if !($invariant) {
            return Err(core::convert::Into::into($error));
        }
    }};
}

/// Ensures two NON-PUBKEY values are equal.
///
/// Use [require_keys_eq] to compare two pubkeys/addresses.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_eq!(count, 0);
///     require_eq!(count, 0, RequireEqViolated);
///     require_eq!(count, 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_eq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 != $value2 {
            $crate::msg!("require_eq violation");
            return Err(core::convert::Into::into($error_code));
        }
    }};
    ($value1:expr, $value2:expr $(,)?) => {{
        $crate::require_eq!($value1, $value2, $crate::ErrorCode::RequireEqViolated);
    }};
}

/// Ensures two NON-PUBKEY values are not equal.
///
/// Use [require_keys_neq] to compare two pubkeys/addresses.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_neq!(count, 0);
///     require_neq!(count, 0, RequireNeqViolated);
///     require_neq!(count, 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_neq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        let __lhs = $value1;
        let __rhs = $value2;
        if __lhs == __rhs {
            $crate::msg!("require_neq violation: left = {}, right = {}", __lhs, __rhs);
            return Err(core::convert::Into::into($error_code));
        }
    }};
    ($value1:expr, $value2:expr $(,)?) => {{
        $crate::require_neq!($value1, $value2, $crate::ErrorCode::RequireNeqViolated)
    }};
}

/// Ensures two pubkey/address values are equal.
///
/// Use [require_eq] to compare two non-pubkey values.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(authority: Address) -> Result<()> {
///     require_keys_eq!(authority, authority);
///     require_keys_eq!(authority, authority, RequireKeysEqViolated);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_keys_eq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 != $value2 {
            $crate::msg!("require_keys_eq violation");
            return Err(core::convert::Into::into($error_code));
        }
    }};
    ($value1:expr, $value2:expr $(,)?) => {{
        $crate::require_keys_eq!($value1, $value2, $crate::ErrorCode::RequireKeysEqViolated);
    }};
}

/// Ensures two pubkey/address values are not equal.
///
/// Use [require_neq] to compare two non-pubkey values.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(authority: Address, other: Address) -> Result<()> {
///     require_keys_neq!(authority, other);
///     require_keys_neq!(authority, other, RequireKeysNeqViolated);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_keys_neq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 == $value2 {
            $crate::msg!("require_keys_neq violation");
            return Err(core::convert::Into::into($error_code));
        }
    }};
    ($value1:expr, $value2:expr $(,)?) => {{
        $crate::require_keys_neq!($value1, $value2, $crate::ErrorCode::RequireKeysNeqViolated);
    }};
}

/// Ensures the first value is greater than the second.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_gt!(count, 0);
///     require_gt!(count, 0, RequireGtViolated);
///     require_gt!(count, 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_gt {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        let __lhs = $value1;
        let __rhs = $value2;
        if __lhs <= __rhs {
            $crate::msg!("require_gt violation: left = {}, right = {}", __lhs, __rhs);
            return Err(core::convert::Into::into($error_code));
        }
    }};
    ($value1:expr, $value2:expr $(,)?) => {{
        $crate::require_gt!($value1, $value2, $crate::ErrorCode::RequireGtViolated)
    }};
}

/// Ensures the first value is greater than or equal to the second.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_gte!(count, 1);
///     require_gte!(count, 1, RequireGteViolated);
///     require_gte!(count, 1, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_gte {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        let __lhs = $value1;
        let __rhs = $value2;
        if __lhs < __rhs {
            $crate::msg!("require_gte violation: left = {}, right = {}", __lhs, __rhs);
            return Err(core::convert::Into::into($error_code));
        }
    }};
    ($value1:expr, $value2:expr $(,)?) => {{
        $crate::require_gte!($value1, $value2, $crate::ErrorCode::RequireGteViolated)
    }};
}
