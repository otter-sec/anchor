//! Prelude: import everything you need with `use anchor_lang_v2::prelude::*;`

#[cfg(feature = "compat")]
pub use crate::solana_program::pubkey::Pubkey;
#[cfg(feature = "compat")]
pub use crate::AccountViewCompat;
pub use crate::{
    access_control,
    account,
    // Account types
    accounts::{
        Account, BorshAccount, Interface, InterfaceAccount, Program, Signer, SlabSchema,
        SystemAccount, Sysvar, SysvarId, UncheckedAccount,
    },
    constant,
    create_account,
    create_account_signed,
    create_program_address,
    // ID
    declare_id,
    declare_program,
    emit,
    error_code,
    // Event
    event,
    find_program_address,
    // Msg
    msg,
    // Pod types
    pod::{PodBool, PodI128, PodI16, PodI32, PodI64, PodU128, PodU16, PodU32, PodU64, PodVec},
    pod_wrapper,
    program,
    // Programs
    programs::{System, Token, Token2022},
    // Require macros (re-exported via #[macro_export])
    require,
    require_eq,
    require_gt,
    require_gte,
    require_keys_eq,
    require_keys_neq,
    require_neq,
    run_handler,
    // Hash
    sha256,
    sol_log_data,
    system_program,
    // Core trait
    AccountAddress,
    // Constraints
    AccountConstraint,
    // Loader & dispatch
    AccountLoader,
    // Client
    AccountMeta as AnchorAccountMeta,
    // Derive macros
    Accounts,
    AnchorAccount,
    Bumps,
    // Context
    Context,
    // CPI
    CpiContext,
    CpiHandle,
    Discriminator,
    Error,
    // Re-export ProgramError for custom error impls
    Error as ProgramError,
    ErrorCode,
    Event,
    Id,
    Ids,
    InitSpace,
    InstructionData,
    Lamports,
    // Nested
    Nested,
    // Marker traits
    Owner,
    // Error
    Result,
    // Serialization
    SchemaRead,
    SchemaWrite,
    Space,
    ToAccountMetas,
    ToCpiAccounts,
    ToCpiHandle,
    ToCpiHandleMut,
    TryAccounts,
};
#[cfg(feature = "event-cpi")]
pub use crate::{emit_cpi, event_cpi};
#[cfg(feature = "compat")]
pub use crate::{err, error, pubkey};
// Re-export pinocchio sysvar types and trait for use with Sysvar<T>
pub use pinocchio::sysvars::Sysvar as PinocchioSysvar;
pub use {
    crate::{IdlAccountType, IdlType},
    pinocchio::{
        account::AccountView,
        address::Address,
        sysvars::{clock::Clock, rent::Rent},
        ProgramResult,
    },
};
