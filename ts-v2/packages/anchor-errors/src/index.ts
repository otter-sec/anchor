// Instruction errors.

/** 8 byte instruction identifier not provided. */
export const ANCHOR_ERROR__INSTRUCTION_MISSING = 100;
/** Fallback functions are not supported. */
export const ANCHOR_ERROR__INSTRUCTION_FALLBACK_NOT_FOUND = 101;
/** The program could not deserialize the given instruction. */
export const ANCHOR_ERROR__INSTRUCTION_DID_NOT_DESERIALIZE = 102;
/** The program could not serialize the given instruction. */
export const ANCHOR_ERROR__INSTRUCTION_DID_NOT_SERIALIZE = 103;

// IDL instruction errors.

/** The program was compiled without idl instructions. */
export const ANCHOR_ERROR__IDL_INSTRUCTION_STUB = 1000;
/** The transaction was given an invalid program for the IDL instruction. */
export const ANCHOR_ERROR__IDL_INSTRUCTION_INVALID_PROGRAM = 1001;
/** IDL account must be empty in order to resize, try closing first. */
export const ANCHOR_ERROR__IDL_ACCOUNT_NOT_EMPTY = 1002;

// Event instructions.

/** The program was compiled without `event-cpi` feature. */
export const ANCHOR_ERROR__EVENT_INSTRUCTION_STUB = 1500;

// Constraint errors.

/** A mut constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MUT = 2000;
/** A has one constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_HAS_ONE = 2001;
/** A signer constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_SIGNER = 2002;
/** A raw constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_RAW = 2003;
/** An owner constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_OWNER = 2004;
/** A rent exemption constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_RENT_EXEMPT = 2005;
/** A seeds constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_SEEDS = 2006;
/** An executable constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_EXECUTABLE = 2007;
/** Deprecated Error, feel free to replace with something else. */
export const ANCHOR_ERROR__CONSTRAINT_STATE = 2008;
/** An associated constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_ASSOCIATED = 2009;
/** An associated init constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_ASSOCIATED_INIT = 2010;
/** A close constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_CLOSE = 2011;
/** An address constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_ADDRESS = 2012;
/** Expected zero account discriminant. */
export const ANCHOR_ERROR__CONSTRAINT_ZERO = 2013;
/** A token mint constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_TOKEN_MINT = 2014;
/** A token owner constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_TOKEN_OWNER = 2015;
/** A mint mint authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_MINT_AUTHORITY = 2016;
/** A mint freeze authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_FREEZE_AUTHORITY = 2017;
/** A mint decimals constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_DECIMALS = 2018;
/** A space constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_SPACE = 2019;
/** A required account for the constraint is None. */
export const ANCHOR_ERROR__CONSTRAINT_ACCOUNT_IS_NONE = 2020;
/** A token account token program constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_TOKEN_TOKEN_PROGRAM = 2021;
/** A mint token program constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_TOKEN_PROGRAM = 2022;
/** An associated token account token program constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_ASSOCIATED_TOKEN_TOKEN_PROGRAM = 2023;
/** A group pointer extension constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION = 2024;
/** A group pointer extension authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION_AUTHORITY = 2025;
/** A group pointer extension group address constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION_GROUP_ADDRESS = 2026;
/** A group member pointer extension constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION = 2027;
/** A group member pointer extension authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION_AUTHORITY = 2028;
/** A group member pointer extension group address constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION_MEMBER_ADDRESS = 2029;
/** A metadata pointer extension constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION = 2030;
/** A metadata pointer extension authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION_AUTHORITY = 2031;
/** A metadata pointer extension metadata address constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION_METADATA_ADDRESS = 2032;
/** A close authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_CLOSE_AUTHORITY_EXTENSION = 2033;
/** A close authority extension authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_CLOSE_AUTHORITY_EXTENSION_AUTHORITY = 2034;
/** A permanent delegate extension constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_PERMANENT_DELEGATE_EXTENSION = 2035;
/** A permanent delegate extension delegate constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_PERMANENT_DELEGATE_EXTENSION_DELEGATE = 2036;
/** A transfer hook extension constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION = 2037;
/** A transfer hook extension authority constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION_AUTHORITY = 2038;
/** A transfer hook extension transfer hook program id constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION_PROGRAM_ID = 2039;
/** A duplicate mutable account constraint was violated. */
export const ANCHOR_ERROR__CONSTRAINT_DUPLICATE_MUTABLE_ACCOUNT = 2040;

// Require errors.

/** A require expression was violated. */
export const ANCHOR_ERROR__REQUIRE_VIOLATED = 2500;
/** A require_eq expression was violated. */
export const ANCHOR_ERROR__REQUIRE_EQ_VIOLATED = 2501;
/** A require_keys_eq expression was violated. */
export const ANCHOR_ERROR__REQUIRE_KEYS_EQ_VIOLATED = 2502;
/** A require_neq expression was violated. */
export const ANCHOR_ERROR__REQUIRE_NEQ_VIOLATED = 2503;
/** A require_keys_neq expression was violated. */
export const ANCHOR_ERROR__REQUIRE_KEYS_NEQ_VIOLATED = 2504;
/** A require_gt expression was violated. */
export const ANCHOR_ERROR__REQUIRE_GT_VIOLATED = 2505;
/** A require_gte expression was violated. */
export const ANCHOR_ERROR__REQUIRE_GTE_VIOLATED = 2506;

// Account errors.

/** The account discriminator was already set on this account. */
export const ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_ALREADY_SET = 3000;
/** No 8 byte discriminator was found on the account. */
export const ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_NOT_FOUND = 3001;
/** 8 byte discriminator did not match what was expected. */
export const ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_MISMATCH = 3002;
/** Failed to deserialize the account. */
export const ANCHOR_ERROR__ACCOUNT_DID_NOT_DESERIALIZE = 3003;
/** Failed to serialize the account. */
export const ANCHOR_ERROR__ACCOUNT_DID_NOT_SERIALIZE = 3004;
/** Not enough account keys given to the instruction. */
export const ANCHOR_ERROR__ACCOUNT_NOT_ENOUGH_KEYS = 3005;
/** The given account is not mutable. */
export const ANCHOR_ERROR__ACCOUNT_NOT_MUTABLE = 3006;
/** The given account is owned by a different program than expected. */
export const ANCHOR_ERROR__ACCOUNT_OWNED_BY_WRONG_PROGRAM = 3007;
/** Program ID was not as expected. */
export const ANCHOR_ERROR__INVALID_PROGRAM_ID = 3008;
/** Program account is not executable. */
export const ANCHOR_ERROR__INVALID_PROGRAM_EXECUTABLE = 3009;
/** The given account did not sign. */
export const ANCHOR_ERROR__ACCOUNT_NOT_SIGNER = 3010;
/** The given account is not owned by the system program. */
export const ANCHOR_ERROR__ACCOUNT_NOT_SYSTEM_OWNED = 3011;
/** The program expected this account to be already initialized. */
export const ANCHOR_ERROR__ACCOUNT_NOT_INITIALIZED = 3012;
/** The given account is not a program data account. */
export const ANCHOR_ERROR__ACCOUNT_NOT_PROGRAM_DATA = 3013;
/** The given account is not the associated token account. */
export const ANCHOR_ERROR__ACCOUNT_NOT_ASSOCIATED_TOKEN_ACCOUNT = 3014;
/** The given public key does not match the required sysvar. */
export const ANCHOR_ERROR__ACCOUNT_SYSVAR_MISMATCH = 3015;
/** The account reallocation exceeds the MAX_PERMITTED_DATA_INCREASE limit. */
export const ANCHOR_ERROR__ACCOUNT_REALLOC_EXCEEDS_LIMIT = 3016;
/** The account was duplicated for more than one reallocation. */
export const ANCHOR_ERROR__ACCOUNT_DUPLICATE_REALLOCS = 3017;

// Miscellaneous errors.

/** The declared program id does not match the actual program id. */
export const ANCHOR_ERROR__DECLARED_PROGRAM_ID_MISMATCH = 4100;
/** You cannot/should not initialize the payer account as a program account. */
export const ANCHOR_ERROR__TRYING_TO_INIT_PAYER_AS_PROGRAM_ACCOUNT = 4101;
/** The program could not perform the numeric conversion, out of range integral type conversion attempted. */
export const ANCHOR_ERROR__INVALID_NUMERIC_CONVERSION = 4102;

// Deprecated errors.

/** The API being used is deprecated and should no longer be used. */
export const ANCHOR_ERROR__DEPRECATED = 5000;

/** All valid Anchor error codes. */
export type AnchorErrorCode =
  | typeof ANCHOR_ERROR__INSTRUCTION_MISSING
  | typeof ANCHOR_ERROR__INSTRUCTION_FALLBACK_NOT_FOUND
  | typeof ANCHOR_ERROR__INSTRUCTION_DID_NOT_DESERIALIZE
  | typeof ANCHOR_ERROR__INSTRUCTION_DID_NOT_SERIALIZE
  | typeof ANCHOR_ERROR__IDL_INSTRUCTION_STUB
  | typeof ANCHOR_ERROR__IDL_INSTRUCTION_INVALID_PROGRAM
  | typeof ANCHOR_ERROR__IDL_ACCOUNT_NOT_EMPTY
  | typeof ANCHOR_ERROR__EVENT_INSTRUCTION_STUB
  | typeof ANCHOR_ERROR__CONSTRAINT_MUT
  | typeof ANCHOR_ERROR__CONSTRAINT_HAS_ONE
  | typeof ANCHOR_ERROR__CONSTRAINT_SIGNER
  | typeof ANCHOR_ERROR__CONSTRAINT_RAW
  | typeof ANCHOR_ERROR__CONSTRAINT_OWNER
  | typeof ANCHOR_ERROR__CONSTRAINT_RENT_EXEMPT
  | typeof ANCHOR_ERROR__CONSTRAINT_SEEDS
  | typeof ANCHOR_ERROR__CONSTRAINT_EXECUTABLE
  | typeof ANCHOR_ERROR__CONSTRAINT_STATE
  | typeof ANCHOR_ERROR__CONSTRAINT_ASSOCIATED
  | typeof ANCHOR_ERROR__CONSTRAINT_ASSOCIATED_INIT
  | typeof ANCHOR_ERROR__CONSTRAINT_CLOSE
  | typeof ANCHOR_ERROR__CONSTRAINT_ADDRESS
  | typeof ANCHOR_ERROR__CONSTRAINT_ZERO
  | typeof ANCHOR_ERROR__CONSTRAINT_TOKEN_MINT
  | typeof ANCHOR_ERROR__CONSTRAINT_TOKEN_OWNER
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_MINT_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_FREEZE_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_DECIMALS
  | typeof ANCHOR_ERROR__CONSTRAINT_SPACE
  | typeof ANCHOR_ERROR__CONSTRAINT_ACCOUNT_IS_NONE
  | typeof ANCHOR_ERROR__CONSTRAINT_TOKEN_TOKEN_PROGRAM
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_TOKEN_PROGRAM
  | typeof ANCHOR_ERROR__CONSTRAINT_ASSOCIATED_TOKEN_TOKEN_PROGRAM
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION_GROUP_ADDRESS
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION_MEMBER_ADDRESS
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION_METADATA_ADDRESS
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_CLOSE_AUTHORITY_EXTENSION
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_CLOSE_AUTHORITY_EXTENSION_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_PERMANENT_DELEGATE_EXTENSION
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_PERMANENT_DELEGATE_EXTENSION_DELEGATE
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION_AUTHORITY
  | typeof ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION_PROGRAM_ID
  | typeof ANCHOR_ERROR__CONSTRAINT_DUPLICATE_MUTABLE_ACCOUNT
  | typeof ANCHOR_ERROR__REQUIRE_VIOLATED
  | typeof ANCHOR_ERROR__REQUIRE_EQ_VIOLATED
  | typeof ANCHOR_ERROR__REQUIRE_KEYS_EQ_VIOLATED
  | typeof ANCHOR_ERROR__REQUIRE_NEQ_VIOLATED
  | typeof ANCHOR_ERROR__REQUIRE_KEYS_NEQ_VIOLATED
  | typeof ANCHOR_ERROR__REQUIRE_GT_VIOLATED
  | typeof ANCHOR_ERROR__REQUIRE_GTE_VIOLATED
  | typeof ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_ALREADY_SET
  | typeof ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_NOT_FOUND
  | typeof ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_MISMATCH
  | typeof ANCHOR_ERROR__ACCOUNT_DID_NOT_DESERIALIZE
  | typeof ANCHOR_ERROR__ACCOUNT_DID_NOT_SERIALIZE
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_ENOUGH_KEYS
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_MUTABLE
  | typeof ANCHOR_ERROR__ACCOUNT_OWNED_BY_WRONG_PROGRAM
  | typeof ANCHOR_ERROR__INVALID_PROGRAM_ID
  | typeof ANCHOR_ERROR__INVALID_PROGRAM_EXECUTABLE
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_SIGNER
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_SYSTEM_OWNED
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_INITIALIZED
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_PROGRAM_DATA
  | typeof ANCHOR_ERROR__ACCOUNT_NOT_ASSOCIATED_TOKEN_ACCOUNT
  | typeof ANCHOR_ERROR__ACCOUNT_SYSVAR_MISMATCH
  | typeof ANCHOR_ERROR__ACCOUNT_REALLOC_EXCEEDS_LIMIT
  | typeof ANCHOR_ERROR__ACCOUNT_DUPLICATE_REALLOCS
  | typeof ANCHOR_ERROR__DECLARED_PROGRAM_ID_MISMATCH
  | typeof ANCHOR_ERROR__TRYING_TO_INIT_PAYER_AS_PROGRAM_ACCOUNT
  | typeof ANCHOR_ERROR__INVALID_NUMERIC_CONVERSION
  | typeof ANCHOR_ERROR__DEPRECATED;
