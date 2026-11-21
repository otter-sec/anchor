//! Error definitions for Anchor v2 programs.

#![no_std]

/// The error returned by an Anchor v2 program.
pub use solana_program_error::ProgramError as Error;

/// The result returned by an Anchor v2 instruction.
pub type Result<T> = core::result::Result<T, Error>;

/// The starting point for user-defined error codes.
pub const ERROR_CODE_OFFSET: u32 = 6000;

/// Errors returned by the Anchor v2 framework.
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
    ConstraintUnique,
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
    /// The program's heap allocator could not satisfy a reservation.
    HeapAllocationFailed,
}

impl From<ErrorCode> for Error {
    #[cold]
    fn from(e: ErrorCode) -> Self {
        match e {
            ErrorCode::AccountNotEnoughKeys => Self::NotEnoughAccountKeys,
            // Custom codes are aligned with Anchor v1's numbering
            ErrorCode::ConstraintMut => Self::Custom(2000),
            ErrorCode::ConstraintSigner => Self::MissingRequiredSignature,
            ErrorCode::ConstraintSeeds => Self::InvalidSeeds,
            ErrorCode::ConstraintHasOne => Self::Custom(2001),
            ErrorCode::ConstraintAddress => Self::Custom(2012),
            ErrorCode::ConstraintAccountIsNone => Self::Custom(2020),
            ErrorCode::ConstraintClose => Self::Custom(2011),
            ErrorCode::ConstraintOwner => Self::IllegalOwner,
            ErrorCode::ConstraintSpace => Self::Custom(2019),
            ErrorCode::ConstraintRentExempt => Self::Custom(2005),
            ErrorCode::ConstraintRaw => Self::Custom(2003),
            ErrorCode::ConstraintExecutable => Self::Custom(2007),
            ErrorCode::ConstraintZero => Self::Custom(2013),
            // Reuses v1's `ConstraintDuplicateMutableAccount` number: the
            // violation is the same — one address in two slots — and v2's
            // mapping table had not claimed 2040.
            ErrorCode::ConstraintUnique => Self::Custom(2040),
            ErrorCode::InstructionDidNotDeserialize => Self::InvalidInstructionData,
            ErrorCode::DeclaredProgramIdMismatch => Self::IncorrectProgramId,
            ErrorCode::InstructionFallbackNotFound => Self::InvalidInstructionData,
            ErrorCode::RequireViolated => Self::Custom(2500),
            ErrorCode::RequireEqViolated => Self::Custom(2501),
            ErrorCode::RequireKeysEqViolated => Self::Custom(2502),
            ErrorCode::RequireNeqViolated => Self::Custom(2503),
            ErrorCode::RequireKeysNeqViolated => Self::Custom(2504),
            ErrorCode::RequireGtViolated => Self::Custom(2505),
            ErrorCode::RequireGteViolated => Self::Custom(2506),
            ErrorCode::HeapAllocationFailed => Self::Custom(4104),
        }
    }
}
