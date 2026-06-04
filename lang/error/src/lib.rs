use {
    anchor_attribute_error::error_code,
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
    std::{
        fmt::{Debug, Display},
        num::TryFromIntError,
    },
};

/// The starting point for user defined error codes.
pub const ERROR_CODE_OFFSET: u32 = 6000;

// `#[error_code]` macro below accesses `anchor_lang::error::...` so set up
// a fake module for it.
mod anchor_lang {
    pub(crate) mod error {
        pub(crate) use crate::{AnchorError, Error};
    }
}

/// Error codes that can be returned by internal framework code.
///
/// - &gt;= 100 Instruction error codes
/// - &gt;= 1000 IDL error codes
/// - &gt;= 2000 constraint error codes
/// - &gt;= 3000 account error codes
/// - &gt;= 4100 misc error codes
/// - = 5000 deprecated error code
///
/// The starting point for user-defined errors is defined
/// by the [ERROR_CODE_OFFSET](crate::error::ERROR_CODE_OFFSET).
#[error_code(offset = 0)]
pub enum ErrorCode {
    // Instructions
    /// 100 - Instruction discriminator not provided
    #[msg("Instruction discriminator not provided")]
    InstructionMissing = 100,
    /// 101 - Fallback functions are not supported
    #[msg("Fallback functions are not supported")]
    InstructionFallbackNotFound,
    /// 102 - The program could not deserialize the given instruction
    #[msg("The program could not deserialize the given instruction")]
    InstructionDidNotDeserialize,
    /// 103 - The program could not serialize the given instruction
    #[msg("The program could not serialize the given instruction")]
    InstructionDidNotSerialize,

    // IDL instructions (deprecated — use Program Metadata instead)
    /// 1000 - The program was compiled without idl instructions
    #[msg("The program was compiled without idl instructions")]
    IdlInstructionStub = 1000,
    /// 1001 - Invalid program given to the IDL instruction
    #[msg("Invalid program given to the IDL instruction")]
    IdlInstructionInvalidProgram,
    /// 1002 - IDL Account must be empty in order to resize
    #[msg("IDL account must be empty in order to resize, try closing first")]
    IdlAccountNotEmpty,

    // Event instructions
    /// 1500 - The program was compiled without `event-cpi` feature
    #[msg("The program was compiled without `event-cpi` feature")]
    EventInstructionStub = 1500,

    // Constraints
    /// 2000 - A mut constraint was violated
    #[msg("A mut constraint was violated")]
    ConstraintMut = 2000,
    /// 2001 - A has one constraint was violated
    #[msg("A has one constraint was violated")]
    ConstraintHasOne,
    /// 2002 - A signer constraint was violated
    #[msg("A signer constraint was violated")]
    ConstraintSigner,
    /// 2003 - A raw constraint was violated
    #[msg("A raw constraint was violated")]
    ConstraintRaw,
    /// 2004 - An owner constraint was violated
    #[msg("An owner constraint was violated")]
    ConstraintOwner,
    /// 2005 - A rent exemption constraint was violated
    #[msg("A rent exemption constraint was violated")]
    ConstraintRentExempt,
    /// 2006 - A seeds constraint was violated
    #[msg("A seeds constraint was violated")]
    ConstraintSeeds,
    /// 2007 - An executable constraint was violated
    #[msg("An executable constraint was violated")]
    ConstraintExecutable,
    /// 2008 - Deprecated Error, feel free to replace with something else
    #[msg("Deprecated Error, feel free to replace with something else")]
    ConstraintState,
    /// 2009 - An associated constraint was violated
    #[msg("An associated constraint was violated")]
    ConstraintAssociated,
    /// 2010 - An associated init constraint was violated
    #[msg("An associated init constraint was violated")]
    ConstraintAssociatedInit,
    /// 2011 - A close constraint was violated
    #[msg("A close constraint was violated")]
    ConstraintClose,
    /// 2012 - An address constraint was violated
    #[msg("An address constraint was violated")]
    ConstraintAddress,
    /// 2013 - Expected zero account discriminant
    #[msg("Expected zero account discriminant")]
    ConstraintZero,
    /// 2014 - A token mint constraint was violated
    #[msg("A token mint constraint was violated")]
    ConstraintTokenMint,
    /// 2015 - A token owner constraint was violated
    #[msg("A token owner constraint was violated")]
    ConstraintTokenOwner,
    /// The mint mint is intentional -> a mint authority for the mint.
    ///
    /// 2016 - A mint mint authority constraint was violated
    #[msg("A mint mint authority constraint was violated")]
    ConstraintMintMintAuthority,
    /// 2017 - A mint freeze authority constraint was violated
    #[msg("A mint freeze authority constraint was violated")]
    ConstraintMintFreezeAuthority,
    /// 2018 - A mint decimals constraint was violated
    #[msg("A mint decimals constraint was violated")]
    ConstraintMintDecimals,
    /// 2019 - A space constraint was violated
    #[msg("A space constraint was violated")]
    ConstraintSpace,
    /// 2020 - A required account for the constraint is None
    #[msg("A required account for the constraint is None")]
    ConstraintAccountIsNone,
    /// The token token is intentional -> a token program for the token account.
    ///
    /// 2021 - A token account token program constraint was violated
    #[msg("A token account token program constraint was violated")]
    ConstraintTokenTokenProgram,
    /// 2022 - A mint token program constraint was violated
    #[msg("A mint token program constraint was violated")]
    ConstraintMintTokenProgram,
    /// 2023 - A mint token program constraint was violated
    #[msg("An associated token account token program constraint was violated")]
    ConstraintAssociatedTokenTokenProgram,
    /// Extension constraints
    ///
    /// 2024 - A group pointer extension constraint was violated
    #[msg("A group pointer extension constraint was violated")]
    ConstraintMintGroupPointerExtension,
    /// 2025 - A group pointer extension authority constraint was violated
    #[msg("A group pointer extension authority constraint was violated")]
    ConstraintMintGroupPointerExtensionAuthority,
    /// 2026 - A group pointer extension group address constraint was violated
    #[msg("A group pointer extension group address constraint was violated")]
    ConstraintMintGroupPointerExtensionGroupAddress,
    /// 2027 - A group member pointer extension constraint was violated
    #[msg("A group member pointer extension constraint was violated")]
    ConstraintMintGroupMemberPointerExtension,
    /// 2028 - A group member pointer extension authority constraint was violated
    #[msg("A group member pointer extension authority constraint was violated")]
    ConstraintMintGroupMemberPointerExtensionAuthority,
    /// 2029 - A group member pointer extension member address constraint was violated
    #[msg("A group member pointer extension group address constraint was violated")]
    ConstraintMintGroupMemberPointerExtensionMemberAddress,
    /// 2030 - A metadata pointer extension constraint was violated
    #[msg("A metadata pointer extension constraint was violated")]
    ConstraintMintMetadataPointerExtension,
    /// 2031 - A metadata pointer extension authority constraint was violated
    #[msg("A metadata pointer extension authority constraint was violated")]
    ConstraintMintMetadataPointerExtensionAuthority,
    /// 2032 - A metadata pointer extension metadata address constraint was violated
    #[msg("A metadata pointer extension metadata address constraint was violated")]
    ConstraintMintMetadataPointerExtensionMetadataAddress,
    /// 2033 - A close authority extension constraint was violated
    #[msg("A close authority constraint was violated")]
    ConstraintMintCloseAuthorityExtension,
    /// 2034 - A close authority extension authority constraint was violated
    #[msg("A close authority extension authority constraint was violated")]
    ConstraintMintCloseAuthorityExtensionAuthority,
    /// 2035 - A permanent delegate extension constraint was violated
    #[msg("A permanent delegate extension constraint was violated")]
    ConstraintMintPermanentDelegateExtension,
    /// 2036 - A permanent delegate extension authority constraint was violated
    #[msg("A permanent delegate extension delegate constraint was violated")]
    ConstraintMintPermanentDelegateExtensionDelegate,
    /// 2037 - A transfer hook extension constraint was violated
    #[msg("A transfer hook extension constraint was violated")]
    ConstraintMintTransferHookExtension,
    /// 2038 - A transfer hook extension authority constraint was violated
    #[msg("A transfer hook extension authority constraint was violated")]
    ConstraintMintTransferHookExtensionAuthority,
    /// 2039 - A transfer hook extension transfer hook program id constraint was violated
    #[msg("A transfer hook extension transfer hook program id constraint was violated")]
    ConstraintMintTransferHookExtensionProgramId,
    /// 2040 - A duplicate mutable account constraint was violated
    #[msg("A duplicate mutable account constraint was violated")]
    ConstraintDuplicateMutableAccount,

    // Migration errors
    /// 2041 - Account is already migrated
    #[msg("Account is already migrated")]
    AccountAlreadyMigrated,
    /// 2042 - Account must be migrated before exiting
    #[msg("Account must be migrated before exiting")]
    AccountNotMigrated,

    /// 2043 - A pausable extension constraint was violated
    #[msg("A pausable extension constraint was violated")]
    ConstraintMintPausableExtension,
    /// 2044 - A pausable extension authority constraint was violated
    #[msg("A pausable extension authority constraint was violated")]
    ConstraintMintPausableAuthority,

    // Require
    /// 2500 - A require expression was violated
    #[msg("A require expression was violated")]
    RequireViolated = 2500,
    /// 2501 - A require_eq expression was violated
    #[msg("A require_eq expression was violated")]
    RequireEqViolated,
    /// 2502 - A require_keys_eq expression was violated
    #[msg("A require_keys_eq expression was violated")]
    RequireKeysEqViolated,
    /// 2503 - A require_neq expression was violated
    #[msg("A require_neq expression was violated")]
    RequireNeqViolated,
    /// 2504 - A require_keys_neq expression was violated
    #[msg("A require_keys_neq expression was violated")]
    RequireKeysNeqViolated,
    /// 2505 - A require_gt expression was violated
    #[msg("A require_gt expression was violated")]
    RequireGtViolated,
    /// 2506 - A require_gte expression was violated
    #[msg("A require_gte expression was violated")]
    RequireGteViolated,

    // Accounts.
    /// 3000 - The account discriminator was already set on this account
    #[msg("The account discriminator was already set on this account")]
    AccountDiscriminatorAlreadySet = 3000,
    /// 3001 - No discriminator was found on the account
    #[msg("No discriminator was found on the account")]
    AccountDiscriminatorNotFound,
    /// 3002 - Account discriminator did not match what was expected
    #[msg("Account discriminator did not match what was expected")]
    AccountDiscriminatorMismatch,
    /// 3003 - Failed to deserialize the account
    #[msg("Failed to deserialize the account")]
    AccountDidNotDeserialize,
    /// 3004 - Failed to serialize the account
    #[msg("Failed to serialize the account")]
    AccountDidNotSerialize,
    /// 3005 - Not enough account keys given to the instruction
    #[msg("Not enough account keys given to the instruction")]
    AccountNotEnoughKeys,
    /// 3006 - The given account is not mutable
    #[msg("The given account is not mutable")]
    AccountNotMutable,
    /// 3007 - The given account is owned by a different program than expected
    #[msg("The given account is owned by a different program than expected")]
    AccountOwnedByWrongProgram,
    /// 3008 - Program ID was not as expected
    #[msg("Program ID was not as expected")]
    InvalidProgramId,
    /// 3009 - Program account is not executable
    #[msg("Program account is not executable")]
    InvalidProgramExecutable,
    /// 3010 - The given account did not sign
    #[msg("The given account did not sign")]
    AccountNotSigner,
    /// 3011 - The given account is not owned by the system program
    #[msg("The given account is not owned by the system program")]
    AccountNotSystemOwned,
    /// 3012 - The program expected this account to be already initialized
    #[msg("The program expected this account to be already initialized")]
    AccountNotInitialized,
    /// 3013 - The given account is not a program data account
    #[msg("The given account is not a program data account")]
    AccountNotProgramData,
    /// 3014 - The given account is not the associated token account
    #[msg("The given account is not the associated token account")]
    AccountNotAssociatedTokenAccount,
    /// 3015 - The given public key does not match the required sysvar
    #[msg("The given public key does not match the required sysvar")]
    AccountSysvarMismatch,
    /// 3016 - The account reallocation exceeds the MAX_PERMITTED_DATA_INCREASE limit
    #[msg("The account reallocation exceeds the MAX_PERMITTED_DATA_INCREASE limit")]
    AccountReallocExceedsLimit,
    /// 3017 - The account was duplicated for more than one reallocation
    #[msg("The account was duplicated for more than one reallocation")]
    AccountDuplicateReallocs,

    // Miscellaneous
    /// 4100 - The declared program id does not match actual program id
    #[msg("The declared program id does not match the actual program id")]
    DeclaredProgramIdMismatch = 4100,
    /// 4101 - You cannot/should not initialize the payer account as a program account
    #[msg("You cannot/should not initialize the payer account as a program account")]
    TryingToInitPayerAsProgramAccount = 4101,
    /// 4102 - Invalid numeric conversion error
    #[msg("Error during numeric conversion")]
    InvalidNumericConversion = 4102,

    // Deprecated
    /// 5000 - The API being used is deprecated and should no longer be used
    #[msg("The API being used is deprecated and should no longer be used")]
    Deprecated = 5000,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    AnchorError(Box<AnchorError>),
    ProgramError(Box<ProgramErrorWithOrigin>),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::AnchorError(ae) => Display::fmt(&ae, f),
            Error::ProgramError(pe) => Display::fmt(&pe, f),
        }
    }
}

impl From<AnchorError> for Error {
    fn from(ae: AnchorError) -> Self {
        Self::AnchorError(Box::new(ae))
    }
}

impl From<ProgramError> for Error {
    fn from(program_error: ProgramError) -> Self {
        Self::ProgramError(Box::new(program_error.into()))
    }
}

#[cfg(feature = "borsh")]
impl From<borsh::io::Error> for Error {
    fn from(error: borsh::io::Error) -> Self {
        Error::ProgramError(Box::new(ProgramError::from(error).into()))
    }
}

impl From<ProgramErrorWithOrigin> for Error {
    fn from(pe: ProgramErrorWithOrigin) -> Self {
        Self::ProgramError(Box::new(pe))
    }
}

impl From<TryFromIntError> for Error {
    fn from(e: TryFromIntError) -> Self {
        Self::AnchorError(Box::new(AnchorError {
            error_name: ErrorCode::InvalidNumericConversion.name(),
            error_code_number: ErrorCode::InvalidNumericConversion.into(),
            error_msg: format!("{e}"),
            error_origin: None,
            compared_values: None,
        }))
    }
}

impl From<std::convert::Infallible> for Error {
    fn from(_: std::convert::Infallible) -> Self {
        unreachable!("Infallible has no inhabitants")
    }
}

impl Error {
    pub fn log(&self) {
        match self {
            Error::ProgramError(program_error) => program_error.log(),
            Error::AnchorError(anchor_error) => anchor_error.log(),
        }
    }

    pub fn with_account_name(mut self, account_name: impl ToString) -> Self {
        match &mut self {
            Error::AnchorError(ae) => {
                ae.error_origin = Some(ErrorOrigin::AccountName(account_name.to_string()));
            }
            Error::ProgramError(pe) => {
                pe.error_origin = Some(ErrorOrigin::AccountName(account_name.to_string()));
            }
        };
        self
    }

    pub fn with_source(mut self, source: Source) -> Self {
        match &mut self {
            Error::AnchorError(ae) => {
                ae.error_origin = Some(ErrorOrigin::Source(source));
            }
            Error::ProgramError(pe) => {
                pe.error_origin = Some(ErrorOrigin::Source(source));
            }
        };
        self
    }

    pub fn with_pubkeys(mut self, pubkeys: (Pubkey, Pubkey)) -> Self {
        let pubkeys = Some(ComparedValues::Pubkeys((pubkeys.0, pubkeys.1)));
        match &mut self {
            Error::AnchorError(ae) => ae.compared_values = pubkeys,
            Error::ProgramError(pe) => pe.compared_values = pubkeys,
        };
        self
    }

    pub fn with_values(mut self, values: (impl ToString, impl ToString)) -> Self {
        match &mut self {
            Error::AnchorError(ae) => {
                ae.compared_values = Some(ComparedValues::Values((
                    values.0.to_string(),
                    values.1.to_string(),
                )))
            }
            Error::ProgramError(pe) => {
                pe.compared_values = Some(ComparedValues::Values((
                    values.0.to_string(),
                    values.1.to_string(),
                )))
            }
        };
        self
    }
}

#[derive(Debug)]
pub struct ProgramErrorWithOrigin {
    pub program_error: ProgramError,
    pub error_origin: Option<ErrorOrigin>,
    pub compared_values: Option<ComparedValues>,
}

// Two ProgramErrors are equal when they have the same error code
impl PartialEq for ProgramErrorWithOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.program_error == other.program_error
    }
}
impl Eq for ProgramErrorWithOrigin {}

impl Display for ProgramErrorWithOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.program_error, f)
    }
}

impl ProgramErrorWithOrigin {
    pub fn log(&self) {
        match &self.error_origin {
            None => {
                solana_msg::msg!(
                    "ProgramError occurred. Error Code: {:?}. Error Number: {}. Error Message: {}.",
                    self.program_error,
                    u64::from(self.program_error.clone()),
                    self.program_error
                );
            }
            Some(ErrorOrigin::Source(source)) => {
                solana_msg::msg!(
                    "ProgramError thrown in {}:{}. Error Code: {:?}. Error Number: {}. Error \
                     Message: {}.",
                    source.filename,
                    source.line,
                    self.program_error,
                    u64::from(self.program_error.clone()),
                    self.program_error
                );
            }
            Some(ErrorOrigin::AccountName(account_name)) => {
                // using sol_log because msg! wrongly interprets 5 inputs as u64
                solana_msg::msg!(&format!(
                    "ProgramError caused by account: {}. Error Code: {:?}. Error Number: {}. \
                     Error Message: {}.",
                    account_name,
                    self.program_error,
                    u64::from(self.program_error.clone()),
                    self.program_error
                ));
            }
        }
        match &self.compared_values {
            Some(ComparedValues::Pubkeys((left, right))) => {
                solana_msg::msg!("Left:");
                left.log();
                solana_msg::msg!("Right:");
                right.log();
            }
            Some(ComparedValues::Values((left, right))) => {
                solana_msg::msg!("Left: {}", left);
                solana_msg::msg!("Right: {}", right);
            }
            None => (),
        }
    }

    pub fn with_source(mut self, source: Source) -> Self {
        self.error_origin = Some(ErrorOrigin::Source(source));
        self
    }

    pub fn with_account_name(mut self, account_name: impl ToString) -> Self {
        self.error_origin = Some(ErrorOrigin::AccountName(account_name.to_string()));
        self
    }
}

impl From<ProgramError> for ProgramErrorWithOrigin {
    fn from(program_error: ProgramError) -> Self {
        Self {
            program_error,
            error_origin: None,
            compared_values: None,
        }
    }
}

#[derive(Debug)]
pub enum ComparedValues {
    Values((String, String)),
    Pubkeys((Pubkey, Pubkey)),
}

#[derive(Debug)]
pub enum ErrorOrigin {
    Source(Source),
    AccountName(String),
}

#[derive(Debug)]
pub struct AnchorError {
    pub error_name: String,
    pub error_code_number: u32,
    pub error_msg: String,
    pub error_origin: Option<ErrorOrigin>,
    pub compared_values: Option<ComparedValues>,
}

impl AnchorError {
    pub fn log(&self) {
        match &self.error_origin {
            None => {
                solana_msg::msg!(
                    "AnchorError occurred. Error Code: {}. Error Number: {}. Error Message: {}.",
                    self.error_name,
                    self.error_code_number,
                    self.error_msg
                );
            }
            Some(ErrorOrigin::Source(source)) => {
                solana_msg::msg!(
                    "AnchorError thrown in {}:{}. Error Code: {}. Error Number: {}. Error \
                     Message: {}.",
                    source.filename,
                    source.line,
                    self.error_name,
                    self.error_code_number,
                    self.error_msg
                );
            }
            Some(ErrorOrigin::AccountName(account_name)) => {
                solana_msg::msg!(
                    "AnchorError caused by account: {}. Error Code: {}. Error Number: {}. Error \
                     Message: {}.",
                    account_name,
                    self.error_name,
                    self.error_code_number,
                    self.error_msg
                );
            }
        }
        match &self.compared_values {
            Some(ComparedValues::Pubkeys((left, right))) => {
                solana_msg::msg!("Left:");
                left.log();
                solana_msg::msg!("Right:");
                right.log();
            }
            Some(ComparedValues::Values((left, right))) => {
                solana_msg::msg!("Left: {}", left);
                solana_msg::msg!("Right: {}", right);
            }
            None => (),
        }
    }

    pub fn with_source(mut self, source: Source) -> Self {
        self.error_origin = Some(ErrorOrigin::Source(source));
        self
    }

    pub fn with_account_name(mut self, account_name: impl ToString) -> Self {
        self.error_origin = Some(ErrorOrigin::AccountName(account_name.to_string()));
        self
    }
}

impl Display for AnchorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

/// Two `AnchorError`s are equal when they have the same error code
impl PartialEq for AnchorError {
    fn eq(&self, other: &Self) -> bool {
        self.error_code_number == other.error_code_number
    }
}

impl Eq for AnchorError {}

impl std::convert::From<Error> for ProgramError {
    fn from(e: Error) -> ProgramError {
        match e {
            Error::AnchorError(error) => ProgramError::Custom(error.error_code_number),
            Error::ProgramError(program_error) => program_error.program_error,
        }
    }
}

#[derive(Debug)]
pub struct Source {
    pub filename: &'static str,
    pub line: u32,
}
