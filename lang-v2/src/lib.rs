//! Anchor v2: Trait-based account system for Solana.
//!
//! `#![no_std]` compatible.

#![no_std]
extern crate alloc;

pub mod accounts;
pub mod context;
mod context_cpi;
pub mod cpi;
pub mod cursor;
mod dispatch;
pub mod event;
pub mod hash;
#[doc(hidden)]
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
#[cfg(feature = "account-resize")]
pub use cpi::realloc_account;
/// Chunked 4×u64 equality compare for `Address`. Preferred over `==`
/// on `&Address`. See <https://github.com/anza-xyz/solana-sdk/issues/345>.
pub use pinocchio::address::address_eq;
#[cfg(feature = "compat")]
pub use solana_address::address as pubkey;
/// Re-export declare_id from solana-address.
pub use solana_address::declare_id;
/// Implementation detail of [`solana_msg`] - re-exported for macro access only.
#[doc(hidden)]
pub use solana_msg;
// Re-export for `debug!` macro — routes through this crate's namespace so
// user programs don't need `solana-program-log` or `extern crate alloc;`.
#[cfg(feature = "compat")]
#[doc(hidden)]
pub use solana_program_log::log as __log_str;

// Ungated re-export so generated macro code (`#[event]`, `debug!`, etc.)
// can reach `Vec` without std or `extern crate alloc;` in user crates.
#[doc(hidden)]
pub extern crate alloc as __alloc;

/// Logs a message via `solana_msg`.
///
/// Thin wrapper around `solana_msg::msg!` that always evaluates to
/// `()`, so it's usable in expression position (match arms, closures, tuples,
/// etc.).
#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {{
        // bring into scope from re-export as the macro accesses it
        use $crate::solana_msg;
        solana_msg::msg!($($arg)*);
    }};
}

/// v1-compat logger with full `format!` support (`{:?}`, `{:x}`, etc.).
///
/// Heap-allocates via `alloc::format!`. Prefer [`msg!`] for production
/// (cheaper CUs). Gated behind `compat`.
///
/// # Example
///
/// ```ignore
/// debug!("raw bytes: {:?}", &data[..32]);
/// ```
#[cfg(feature = "compat")]
#[macro_export]
macro_rules! debug {
    ($msg:expr) => {{
        $crate::__log_str($msg)
    }};
    ($($arg:tt)*) => {{
        $crate::__log_str(&$crate::__alloc::format!($($arg)*))
    }};
}
// Re-export wincode for instruction data serialization
pub use wincode;

/// Borsh-compatible wincode config: u8 enum tags + fixed u32 LE length
/// prefixes. Used for all serialization in v2 (instruction args, events,
/// `BorshAccount<T>`) so the on-chain wire format matches borsh exactly,
/// while keeping wincode's faster encoding path.
///
/// # ⚠ Incompatibilities with the borsh crate
///
/// Wincode with this config is byte-identical to borsh for the shapes Anchor
/// programs commonly use (integers, fixed arrays, `Vec`, `String`, `Option`,
/// tagged enums, nested structs). The following shapes are NOT byte-
/// identical — if a program built on Anchor v1 (real borsh) used them, the
/// on-chain bytes will NOT round-trip cleanly through v2:
///
/// - **`HashMap` / `HashSet`**: borsh sorts entries by key, wincode preserves
///   insertion order. Use `BTreeMap` / `BTreeSet` or `Vec<(K, V)>` if you
///   need canonical ordering.
/// - **`f32` / `f64` NaN**: borsh rejects NaN on deserialize, wincode
///   accepts it. v2 won't surface an error for a NaN-bearing account.
///
/// Programs that don't use these types are unaffected.
///
/// # `ZERO_COPY_ALIGN_CHECK = false`
///
/// Borsh's u32 length prefix puts payload data 4 bytes off natural alignment,
/// so handler args like `amounts: &[u64]` would otherwise fail wincode's
/// runtime alignment guard. The guard exists to prevent Rust-level UB on
/// hosts where misaligned wide loads are undefined; SBPF's `ldxdw` has no
/// alignment-specialized variants and the Solana program ecosystem already
/// reads u64 from arbitrary `&[u8]` offsets, so disabling the check on SBPF
/// is safe.
pub const BORSH_CONFIG: BorshConfig = wincode::config::Configuration::new();

/// Concrete type of [`BORSH_CONFIG`]. Spelled out so downstream callers can
/// name it in trait bounds (e.g. `T: wincode::SchemaRead<'de, BorshConfig>`).
pub type BorshConfig = wincode::config::Configuration<
    false,
    { wincode::config::DEFAULT_PREALLOCATION_SIZE_LIMIT },
    wincode::len::FixIntLen<u32>,
    wincode::int_encoding::LittleEndian,
    wincode::int_encoding::FixInt,
    u8,
>;

/// `#[derive(IdlType)]` — register a plain struct in the IDL's `types[]`
/// array.
///
/// **Opaque / unstable.** Apply this derive on user types you want to
/// surface in the generated IDL; do not call any of the emitted associated
/// items directly — they are implementation details of the `anchor idl
/// build` pipeline and will change without notice. The emitted impl body
/// is gated on the **end-user crate's** local `idl-build` feature, so
/// non-IDL builds pay nothing.
pub use anchor_derive_accounts_v2::IdlType;
/// **Opaque / unstable.** Re-exported so derive-emitted code in user
/// crates can name the trait. Do not implement this trait by hand or call
/// its associated items — they are implementation details of the IDL
/// build pipeline and will change without notice. See [`idl_build`] for
/// the trait definition.
pub use idl_build::IdlAccountType;
// ---------------------------------------------------------------------------
// Client-side types — for building instructions off-chain (tests, CPI, SDK)
// ---------------------------------------------------------------------------
#[cfg(feature = "event-cpi")]
pub use anchor_derive_accounts_v2::{emit_cpi, event_cpi};
/// Metadata for a single account in a transaction instruction.
///
/// Re-exported from `solana-instruction` so tests and CPI builders can pass
/// the output of `to_account_metas()` straight into `solana_instruction::
/// Instruction::new_with_bytes` without a manual field rename.
///
/// `AccountMeta` itself comes from `solana-instruction`, whose account key
/// field is named `solana_pubkey::Pubkey`. In current Solana crates that
/// `Pubkey` is a re-export of `solana_address::Address`, so built-in program
/// marker IDs such as `Token::id()` already return the same address type and
/// do not need a byte-level conversion.
pub use solana_instruction::account_meta::AccountMeta;
pub use {
    accounts::{AccountInitialize, SlabInit},
    anchor_derive_accounts_v2::{
        access_control, account, constant, declare_program, emit, error_code, event, pod_wrapper,
        program, Accounts, InitSpace,
    },
    bytemuck,
    context::{Bumps, Context, MutMask},
    context_cpi::{invoke_signed_fixed, CpiContext},
    cpi::{
        create_account, create_account_signed, create_program_address,
        find_and_verify_program_address, find_and_verify_program_address_skip_curve,
        find_program_address, verify_program_address,
    },
    cursor::{mut_mask_or_shifted, mut_mask_set_bit, AccountBitvec, AccountCursor},
    dispatch::{run_handler, TryAccounts},
    event::{sol_log_data, Event},
    hash::sha256,
    loader::AccountLoader,
    pinocchio::{self, account::AccountView, address::Address},
    traits::*,
    wincode::{SchemaRead, SchemaWrite},
};

/// Re-export of the Solana SDK `Instruction` + `AccountMeta` types under a v1-
/// compatible module path. Lets users write
/// `use anchor_lang_v2::solana_program::instruction::{Instruction, AccountMeta}`
/// without adding `solana-instruction` to their `Cargo.toml`.
pub mod solana_program {
    pub mod clock {
        pub use pinocchio::sysvars::clock::*;
    }

    pub mod hash {
        pub use solana_sha256_hasher::*;
    }

    pub mod instruction {
        pub use solana_instruction::*;
    }

    pub use solana_system_interface::instruction as system_instruction;

    #[cfg(feature = "compat")]
    pub use crate::pubkey;

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

/// Converts a struct of account addresses into a list of [`AccountMeta`]s.
pub trait ToAccountMetas {
    fn to_account_metas(&self, is_signer: Option<bool>) -> alloc::vec::Vec<AccountMeta>;
}

/// Serializes instruction data: discriminator prefix + LE-encoded args.
pub trait InstructionData: Discriminator {
    fn data(&self) -> alloc::vec::Vec<u8>;
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

#[doc(hidden)]
pub mod __private {
    /// Used by `#[derive(InitSpace)]` on enums to pick the largest variant size.
    pub const fn max(a: usize, b: usize) -> usize {
        [a, b][(a < b) as usize]
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
    ConstraintClose,
    ConstraintOwner,
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
    ConstraintDuplicateMutableAccount,
}

impl From<ErrorCode> for solana_program_error::ProgramError {
    #[cold]
    fn from(e: ErrorCode) -> Self {
        match e {
            ErrorCode::AccountNotEnoughKeys => {
                solana_program_error::ProgramError::NotEnoughAccountKeys
            }
            ErrorCode::ConstraintMut => solana_program_error::ProgramError::Custom(2000),
            ErrorCode::ConstraintSigner => {
                solana_program_error::ProgramError::MissingRequiredSignature
            }
            ErrorCode::ConstraintSeeds => solana_program_error::ProgramError::InvalidSeeds,
            ErrorCode::ConstraintHasOne => solana_program_error::ProgramError::InvalidAccountData,
            ErrorCode::ConstraintAddress => solana_program_error::ProgramError::InvalidAccountData,
            ErrorCode::ConstraintClose => solana_program_error::ProgramError::InvalidAccountData,
            ErrorCode::ConstraintOwner => solana_program_error::ProgramError::IllegalOwner,
            ErrorCode::ConstraintRaw => solana_program_error::ProgramError::Custom(2001),
            ErrorCode::ConstraintExecutable => solana_program_error::ProgramError::Custom(2002),
            ErrorCode::ConstraintZero => solana_program_error::ProgramError::Custom(2004),
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
            ErrorCode::RequireNeqViolated => solana_program_error::ProgramError::Custom(2502),
            ErrorCode::RequireKeysEqViolated => solana_program_error::ProgramError::Custom(2503),
            ErrorCode::RequireKeysNeqViolated => solana_program_error::ProgramError::Custom(2504),
            ErrorCode::RequireGtViolated => solana_program_error::ProgramError::Custom(2505),
            ErrorCode::RequireGteViolated => solana_program_error::ProgramError::Custom(2506),
            ErrorCode::ConstraintDuplicateMutableAccount => {
                solana_program_error::ProgramError::Custom(2005)
            }
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
/// # use anchor_lang_v2::prelude::*;
/// fn check(amount: u64) -> Result<()> {
///     require!(amount > 0, ConstraintRaw);
///     require!(amount > 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require {
    ($invariant:expr, $error:tt $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if !($invariant) {
            return Err($crate::ErrorCode::$error.into());
        }
    };
    ($invariant:expr, $error:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if !($invariant) {
            return Err(core::convert::Into::into($error));
        }
    };
}

/// Ensures two NON-PUBKEY values are equal.
///
/// Use [require_keys_eq] to compare two pubkeys/addresses.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang_v2::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_eq!(count, 0);
///     require_eq!(count, 0, RequireEqViolated);
///     require_eq!(count, 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_eq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 != $value2 {
            $crate::msg!(
                "require_eq violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err(core::convert::Into::into($error_code));
        }
    };
    ($value1:expr, $value2:expr $(,)?) => {
        if $value1 != $value2 {
            $crate::msg!(
                "require_eq violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err($crate::ErrorCode::RequireEqViolated.into());
        }
    };
}

/// Ensures two NON-PUBKEY values are not equal.
///
/// Use [require_keys_neq] to compare two pubkeys/addresses.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang_v2::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_neq!(count, 0);
///     require_neq!(count, 0, RequireNeqViolated);
///     require_neq!(count, 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_neq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 == $value2 {
            $crate::msg!(
                "require_neq violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err(core::convert::Into::into($error_code));
        }
    };
    ($value1:expr, $value2:expr $(,)?) => {
        if $value1 == $value2 {
            $crate::msg!(
                "require_neq violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err($crate::ErrorCode::RequireNeqViolated.into());
        }
    };
}

/// Ensures two pubkey/address values are equal.
///
/// Use [require_eq] to compare two non-pubkey values.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang_v2::prelude::*;
/// fn check(authority: Address) -> Result<()> {
///     require_keys_eq!(authority, authority);
///     require_keys_eq!(authority, authority, RequireKeysEqViolated);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_keys_eq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 != $value2 {
            $crate::msg!("require_keys_eq violation");
            return Err(core::convert::Into::into($error_code));
        }
    };
    ($value1:expr, $value2:expr $(,)?) => {
        if $value1 != $value2 {
            $crate::msg!("require_keys_eq violation");
            return Err($crate::ErrorCode::RequireKeysEqViolated.into());
        }
    };
}

/// Ensures two pubkey/address values are not equal.
///
/// Use [require_neq] to compare two non-pubkey values.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang_v2::prelude::*;
/// fn check(authority: Address, other: Address) -> Result<()> {
///     require_keys_neq!(authority, other);
///     require_keys_neq!(authority, other, RequireKeysNeqViolated);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_keys_neq {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 == $value2 {
            $crate::msg!("require_keys_neq violation");
            return Err(core::convert::Into::into($error_code));
        }
    };
    ($value1:expr, $value2:expr $(,)?) => {
        if $value1 == $value2 {
            $crate::msg!("require_keys_neq violation");
            return Err($crate::ErrorCode::RequireKeysNeqViolated.into());
        }
    };
}

/// Ensures the first value is greater than the second.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang_v2::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_gt!(count, 0);
///     require_gt!(count, 0, RequireGtViolated);
///     require_gt!(count, 0, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_gt {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 <= $value2 {
            $crate::msg!(
                "require_gt violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err(core::convert::Into::into($error_code));
        }
    };
    ($value1:expr, $value2:expr $(,)?) => {
        if $value1 <= $value2 {
            $crate::msg!(
                "require_gt violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err($crate::ErrorCode::RequireGtViolated.into());
        }
    };
}

/// Ensures the first value is greater than or equal to the second.
///
/// # Example
/// ```rust,no_run
/// # use anchor_lang_v2::prelude::*;
/// fn check(count: u64) -> Result<()> {
///     require_gte!(count, 1);
///     require_gte!(count, 1, RequireGteViolated);
///     require_gte!(count, 1, ProgramError::InvalidArgument);
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! require_gte {
    ($value1:expr, $value2:expr, $error_code:expr $(,)?) => {
        #[allow(unused_imports)]
        use $crate::ErrorCode::*;
        if $value1 < $value2 {
            $crate::msg!(
                "require_gte violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err(core::convert::Into::into($error_code));
        }
    };
    ($value1:expr, $value2:expr $(,)?) => {
        if $value1 < $value2 {
            $crate::msg!(
                "require_gte violation: left = {}, right = {}",
                $value1,
                $value2
            );
            return Err($crate::ErrorCode::RequireGteViolated.into());
        }
    };
}
