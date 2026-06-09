import { PublicKey } from "@solana/web3.js";
import * as errors from "@anchor-lang/errors";
import * as features from "./utils/features.js";

export class IdlError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "IdlError";
  }
}

interface ErrorCode {
  code: string;
  number: number;
}

interface FileLine {
  file: string;
  line: number;
}

type Origin = string | FileLine;
type ComparedAccountNames = [string, string];
type ComparedPublicKeys = [PublicKey, PublicKey];
type ComparedValues = ComparedAccountNames | ComparedPublicKeys;

export class ProgramErrorStack {
  constructor(readonly stack: PublicKey[]) {}

  public static parse(logs: string[]) {
    const programKeyRegex = /^Program (\w*) invoke/;
    const successRegex = /^Program \w* success/;

    const programStack: PublicKey[] = [];
    for (let i = 0; i < logs.length; i++) {
      if (successRegex.exec(logs[i])) {
        programStack.pop();
        continue;
      }

      const programKey = programKeyRegex.exec(logs[i])?.[1];
      if (!programKey) {
        continue;
      }
      programStack.push(new PublicKey(programKey));
    }
    return new ProgramErrorStack(programStack);
  }
}

export class AnchorError extends Error {
  readonly error: {
    errorCode: ErrorCode;
    errorMessage: string;
    comparedValues?: ComparedValues;
    origin?: Origin;
  };
  private readonly _programErrorStack: ProgramErrorStack;

  constructor(
    errorCode: ErrorCode,
    errorMessage: string,
    readonly errorLogs: string[],
    readonly logs: string[],
    origin?: Origin,
    comparedValues?: ComparedValues
  ) {
    super(errorLogs.join("\n").replace("Program log: ", ""));
    this.error = { errorCode, errorMessage, comparedValues, origin };
    this._programErrorStack = ProgramErrorStack.parse(logs);
  }

  public static parse(logs: string[]) {
    if (!logs) {
      return null;
    }

    const anchorErrorLogIndex = logs.findIndex((log) =>
      log.startsWith("Program log: AnchorError")
    );
    if (anchorErrorLogIndex === -1) {
      return null;
    }
    const anchorErrorLog = logs[anchorErrorLogIndex];
    const errorLogs = [anchorErrorLog];
    let comparedValues: ComparedValues | undefined;
    if (anchorErrorLogIndex + 1 < logs.length) {
      // This catches the comparedValues where the following is logged
      // <AnchorError>
      // Left:
      // <Pubkey>
      // Right:
      // <Pubkey>
      if (logs[anchorErrorLogIndex + 1] === "Program log: Left:") {
        const pubkeyRegex = /^Program log: (.*)$/;
        const leftPubkey = pubkeyRegex.exec(logs[anchorErrorLogIndex + 2])![1];
        const rightPubkey = pubkeyRegex.exec(logs[anchorErrorLogIndex + 4])![1];
        comparedValues = [
          new PublicKey(leftPubkey),
          new PublicKey(rightPubkey),
        ];
        errorLogs.push(
          ...logs.slice(anchorErrorLogIndex + 1, anchorErrorLogIndex + 5)
        );
      }
      // This catches the comparedValues where the following is logged
      // <AnchorError>
      // Left: <value>
      // Right: <value>
      else if (logs[anchorErrorLogIndex + 1].startsWith("Program log: Left:")) {
        const valueRegex = /^Program log: (Left|Right): (.*)$/;
        const leftValue = valueRegex.exec(logs[anchorErrorLogIndex + 1])![2];
        const rightValue = valueRegex.exec(logs[anchorErrorLogIndex + 2])![2];
        errorLogs.push(
          ...logs.slice(anchorErrorLogIndex + 1, anchorErrorLogIndex + 3)
        );
        comparedValues = [leftValue, rightValue];
      }
    }
    const regexNoInfo =
      /^Program log: AnchorError occurred\. Error Code: (.*)\. Error Number: (\d*)\. Error Message: (.*)\./;
    const noInfoAnchorErrorLog = regexNoInfo.exec(anchorErrorLog);
    const regexFileLine =
      /^Program log: AnchorError thrown in (.*):(\d*)\. Error Code: (.*)\. Error Number: (\d*)\. Error Message: (.*)\./;
    const fileLineAnchorErrorLog = regexFileLine.exec(anchorErrorLog);
    const regexAccountName =
      /^Program log: AnchorError caused by account: (.*)\. Error Code: (.*)\. Error Number: (\d*)\. Error Message: (.*)\./;
    const accountNameAnchorErrorLog = regexAccountName.exec(anchorErrorLog);
    if (noInfoAnchorErrorLog) {
      const [errorCodeString, errorNumber, errorMessage] =
        noInfoAnchorErrorLog.slice(1, 4);
      const errorCode = {
        code: errorCodeString,
        number: parseInt(errorNumber),
      };
      return new AnchorError(
        errorCode,
        errorMessage,
        errorLogs,
        logs,
        undefined,
        comparedValues
      );
    } else if (fileLineAnchorErrorLog) {
      const [file, line, errorCodeString, errorNumber, errorMessage] =
        fileLineAnchorErrorLog.slice(1, 6);
      const errorCode = {
        code: errorCodeString,
        number: parseInt(errorNumber),
      };
      const fileLine = { file, line: parseInt(line) };
      return new AnchorError(
        errorCode,
        errorMessage,
        errorLogs,
        logs,
        fileLine,
        comparedValues
      );
    } else if (accountNameAnchorErrorLog) {
      const [accountName, errorCodeString, errorNumber, errorMessage] =
        accountNameAnchorErrorLog.slice(1, 5);
      const origin = accountName;
      const errorCode = {
        code: errorCodeString,
        number: parseInt(errorNumber),
      };
      return new AnchorError(
        errorCode,
        errorMessage,
        errorLogs,
        logs,
        origin,
        comparedValues
      );
    } else {
      return null;
    }
  }

  get program(): PublicKey {
    return this._programErrorStack.stack[
      this._programErrorStack.stack.length - 1
    ];
  }

  get programErrorStack(): PublicKey[] {
    return this._programErrorStack.stack;
  }

  public toString(): string {
    return this.message;
  }
}

// An error from a user defined program.
export class ProgramError extends Error {
  private readonly _programErrorStack?: ProgramErrorStack;

  constructor(
    readonly code: number,
    readonly msg: string,
    readonly logs?: string[]
  ) {
    super();
    if (logs) {
      this._programErrorStack = ProgramErrorStack.parse(logs);
    }
  }

  public static parse(
    err: any,
    idlErrors: Map<number, string>
  ): ProgramError | null {
    const errString: string = err.toString();
    // TODO: don't rely on the error string. web3.js should preserve the error
    //       code information instead of giving us an untyped string.
    let unparsedErrorCode: string;
    if (errString.includes("custom program error:")) {
      let components = errString.split("custom program error: ");
      if (components.length !== 2) {
        return null;
      } else {
        unparsedErrorCode = components[1];
      }
    } else {
      const matches = errString.match(/"Custom":([0-9]+)}/g);
      if (!matches || matches.length > 1) {
        return null;
      }
      unparsedErrorCode = matches[0].match(/([0-9]+)/g)![0];
    }

    let errorCode: number;
    try {
      errorCode = parseInt(unparsedErrorCode);
    } catch (parseErr) {
      return null;
    }

    // Parse user error.
    let errorMsg = idlErrors.get(errorCode);
    if (errorMsg !== undefined) {
      return new ProgramError(errorCode, errorMsg, err.logs);
    }

    // Parse framework internal error.
    errorMsg = LangErrorMessage.get(errorCode);
    if (errorMsg !== undefined) {
      return new ProgramError(errorCode, errorMsg, err.logs);
    }

    // Unable to parse the error. Just return the untranslated error.
    return null;
  }

  get program(): PublicKey | undefined {
    return this._programErrorStack?.stack[
      this._programErrorStack.stack.length - 1
    ];
  }

  get programErrorStack(): PublicKey[] | undefined {
    return this._programErrorStack?.stack;
  }

  public toString(): string {
    return this.msg;
  }
}

export function translateError(err: any, idlErrors: Map<number, string>) {
  if (features.isSet("debug-logs")) {
    console.log("Translating error:", err);
  }

  const anchorError = AnchorError.parse(err.logs);
  if (anchorError) {
    return anchorError;
  }

  const programError = ProgramError.parse(err, idlErrors);
  if (programError) {
    return programError;
  }
  if (err.logs) {
    const handler = {
      get: function (target, prop) {
        if (prop === "programErrorStack") {
          return target.programErrorStack.stack;
        } else if (prop === "program") {
          return target.programErrorStack.stack[
            err.programErrorStack.stack.length - 1
          ];
        } else {
          // this is the normal way to return all other props
          // without modifying them.
          // @ts-expect-error
          return Reflect.get(...arguments);
        }
      },
    };
    err.programErrorStack = ProgramErrorStack.parse(err.logs);
    return new Proxy(err, handler);
  }
  return err;
}

export const LangErrorCode = {
  // Instructions.
  InstructionMissing: errors.ANCHOR_ERROR__INSTRUCTION_MISSING,
  InstructionFallbackNotFound:
    errors.ANCHOR_ERROR__INSTRUCTION_FALLBACK_NOT_FOUND,
  InstructionDidNotDeserialize:
    errors.ANCHOR_ERROR__INSTRUCTION_DID_NOT_DESERIALIZE,
  InstructionDidNotSerialize:
    errors.ANCHOR_ERROR__INSTRUCTION_DID_NOT_SERIALIZE,

  // IDL instructions.
  /** @deprecated Legacy IDL error codes. Use Program Metadata instead. Will be removed in a future release. */
  IdlInstructionStub: errors.ANCHOR_ERROR__IDL_INSTRUCTION_STUB,
  /** @deprecated */
  IdlInstructionInvalidProgram:
    errors.ANCHOR_ERROR__IDL_INSTRUCTION_INVALID_PROGRAM,
  /** @deprecated */
  IdlAccountNotEmpty: errors.ANCHOR_ERROR__IDL_ACCOUNT_NOT_EMPTY,

  // Event instructions.
  EventInstructionStub: errors.ANCHOR_ERROR__EVENT_INSTRUCTION_STUB,

  // Constraints.
  ConstraintMut: errors.ANCHOR_ERROR__CONSTRAINT_MUT,
  ConstraintHasOne: errors.ANCHOR_ERROR__CONSTRAINT_HAS_ONE,
  ConstraintSigner: errors.ANCHOR_ERROR__CONSTRAINT_SIGNER,
  ConstraintRaw: errors.ANCHOR_ERROR__CONSTRAINT_RAW,
  ConstraintOwner: errors.ANCHOR_ERROR__CONSTRAINT_OWNER,
  ConstraintRentExempt: errors.ANCHOR_ERROR__CONSTRAINT_RENT_EXEMPT,
  ConstraintSeeds: errors.ANCHOR_ERROR__CONSTRAINT_SEEDS,
  ConstraintExecutable: errors.ANCHOR_ERROR__CONSTRAINT_EXECUTABLE,
  ConstraintState: errors.ANCHOR_ERROR__CONSTRAINT_STATE,
  ConstraintAssociated: errors.ANCHOR_ERROR__CONSTRAINT_ASSOCIATED,
  ConstraintAssociatedInit: errors.ANCHOR_ERROR__CONSTRAINT_ASSOCIATED_INIT,
  ConstraintClose: errors.ANCHOR_ERROR__CONSTRAINT_CLOSE,
  ConstraintAddress: errors.ANCHOR_ERROR__CONSTRAINT_ADDRESS,
  ConstraintZero: errors.ANCHOR_ERROR__CONSTRAINT_ZERO,
  ConstraintTokenMint: errors.ANCHOR_ERROR__CONSTRAINT_TOKEN_MINT,
  ConstraintTokenOwner: errors.ANCHOR_ERROR__CONSTRAINT_TOKEN_OWNER,
  ConstraintMintMintAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_MINT_AUTHORITY,
  ConstraintMintFreezeAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_FREEZE_AUTHORITY,
  ConstraintMintDecimals: errors.ANCHOR_ERROR__CONSTRAINT_MINT_DECIMALS,
  ConstraintSpace: errors.ANCHOR_ERROR__CONSTRAINT_SPACE,
  ConstraintAccountIsNone: errors.ANCHOR_ERROR__CONSTRAINT_ACCOUNT_IS_NONE,
  ConstraintTokenTokenProgram:
    errors.ANCHOR_ERROR__CONSTRAINT_TOKEN_TOKEN_PROGRAM,
  ConstraintMintTokenProgram:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_TOKEN_PROGRAM,
  ConstraintAssociatedTokenTokenProgram:
    errors.ANCHOR_ERROR__CONSTRAINT_ASSOCIATED_TOKEN_TOKEN_PROGRAM,
  ConstraintMintGroupPointerExtension:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION,
  ConstraintMintGroupPointerExtensionAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION_AUTHORITY,
  ConstraintMintGroupPointerExtensionGroupAddress:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_POINTER_EXTENSION_GROUP_ADDRESS,
  ConstraintMintGroupMemberPointerExtension:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION,
  ConstraintMintGroupMemberPointerExtensionAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION_AUTHORITY,
  ConstraintMintGroupMemberPointerExtensionMemberAddress:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_GROUP_MEMBER_POINTER_EXTENSION_MEMBER_ADDRESS,
  ConstraintMintMetadataPointerExtension:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION,
  ConstraintMintMetadataPointerExtensionAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION_AUTHORITY,
  ConstraintMintMetadataPointerExtensionMetadataAddress:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_METADATA_POINTER_EXTENSION_METADATA_ADDRESS,
  ConstraintMintCloseAuthorityExtension:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_CLOSE_AUTHORITY_EXTENSION,
  ConstraintMintCloseAuthorityExtensionAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_CLOSE_AUTHORITY_EXTENSION_AUTHORITY,
  ConstraintMintPermanentDelegateExtension:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_PERMANENT_DELEGATE_EXTENSION,
  ConstraintMintPermanentDelegateExtensionDelegate:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_PERMANENT_DELEGATE_EXTENSION_DELEGATE,
  ConstraintMintTransferHookExtension:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION,
  ConstraintMintTransferHookExtensionAuthority:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION_AUTHORITY,
  ConstraintMintTransferHookExtensionProgramId:
    errors.ANCHOR_ERROR__CONSTRAINT_MINT_TRANSFER_HOOK_EXTENSION_PROGRAM_ID,

  // Require.
  RequireViolated: errors.ANCHOR_ERROR__REQUIRE_VIOLATED,
  RequireEqViolated: errors.ANCHOR_ERROR__REQUIRE_EQ_VIOLATED,
  RequireKeysEqViolated: errors.ANCHOR_ERROR__REQUIRE_KEYS_EQ_VIOLATED,
  RequireNeqViolated: errors.ANCHOR_ERROR__REQUIRE_NEQ_VIOLATED,
  RequireKeysNeqViolated: errors.ANCHOR_ERROR__REQUIRE_KEYS_NEQ_VIOLATED,
  RequireGtViolated: errors.ANCHOR_ERROR__REQUIRE_GT_VIOLATED,
  RequireGteViolated: errors.ANCHOR_ERROR__REQUIRE_GTE_VIOLATED,

  // Accounts.
  AccountDiscriminatorAlreadySet:
    errors.ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_ALREADY_SET,
  AccountDiscriminatorNotFound:
    errors.ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_NOT_FOUND,
  AccountDiscriminatorMismatch:
    errors.ANCHOR_ERROR__ACCOUNT_DISCRIMINATOR_MISMATCH,
  AccountDidNotDeserialize: errors.ANCHOR_ERROR__ACCOUNT_DID_NOT_DESERIALIZE,
  AccountDidNotSerialize: errors.ANCHOR_ERROR__ACCOUNT_DID_NOT_SERIALIZE,
  AccountNotEnoughKeys: errors.ANCHOR_ERROR__ACCOUNT_NOT_ENOUGH_KEYS,
  AccountNotMutable: errors.ANCHOR_ERROR__ACCOUNT_NOT_MUTABLE,
  AccountOwnedByWrongProgram:
    errors.ANCHOR_ERROR__ACCOUNT_OWNED_BY_WRONG_PROGRAM,
  InvalidProgramId: errors.ANCHOR_ERROR__INVALID_PROGRAM_ID,
  InvalidProgramExecutable: errors.ANCHOR_ERROR__INVALID_PROGRAM_EXECUTABLE,
  AccountNotSigner: errors.ANCHOR_ERROR__ACCOUNT_NOT_SIGNER,
  AccountNotSystemOwned: errors.ANCHOR_ERROR__ACCOUNT_NOT_SYSTEM_OWNED,
  AccountNotInitialized: errors.ANCHOR_ERROR__ACCOUNT_NOT_INITIALIZED,
  AccountNotProgramData: errors.ANCHOR_ERROR__ACCOUNT_NOT_PROGRAM_DATA,
  AccountNotAssociatedTokenAccount:
    errors.ANCHOR_ERROR__ACCOUNT_NOT_ASSOCIATED_TOKEN_ACCOUNT,
  AccountSysvarMismatch: errors.ANCHOR_ERROR__ACCOUNT_SYSVAR_MISMATCH,
  AccountReallocExceedsLimit:
    errors.ANCHOR_ERROR__ACCOUNT_REALLOC_EXCEEDS_LIMIT,
  AccountDuplicateReallocs: errors.ANCHOR_ERROR__ACCOUNT_DUPLICATE_REALLOCS,

  // Miscellaneous
  DeclaredProgramIdMismatch: errors.ANCHOR_ERROR__DECLARED_PROGRAM_ID_MISMATCH,
  TryingToInitPayerAsProgramAccount:
    errors.ANCHOR_ERROR__TRYING_TO_INIT_PAYER_AS_PROGRAM_ACCOUNT,
  InvalidNumericConversion: errors.ANCHOR_ERROR__INVALID_NUMERIC_CONVERSION,

  // Used for APIs that shouldn't be used anymore.
  Deprecated: errors.ANCHOR_ERROR__DEPRECATED,
};

export const LangErrorMessage = new Map<number, string>([
  // Instructions.
  [LangErrorCode.InstructionMissing, "Instruction discriminator not provided"],
  [
    LangErrorCode.InstructionFallbackNotFound,
    "Fallback functions are not supported",
  ],
  [
    LangErrorCode.InstructionDidNotDeserialize,
    "The program could not deserialize the given instruction",
  ],
  [
    LangErrorCode.InstructionDidNotSerialize,
    "The program could not serialize the given instruction",
  ],

  // Idl instructions.
  [
    LangErrorCode.IdlInstructionStub,
    "The program was compiled without idl instructions",
  ],
  [
    LangErrorCode.IdlInstructionInvalidProgram,
    "The transaction was given an invalid program for the IDL instruction",
  ],
  [
    LangErrorCode.IdlAccountNotEmpty,
    "IDL account must be empty in order to resize, try closing first",
  ],

  // Event instructions.
  [
    LangErrorCode.EventInstructionStub,
    "The program was compiled without `event-cpi` feature",
  ],

  // Constraints.
  [LangErrorCode.ConstraintMut, "A mut constraint was violated"],
  [LangErrorCode.ConstraintHasOne, "A has one constraint was violated"],
  [LangErrorCode.ConstraintSigner, "A signer constraint was violated"],
  [LangErrorCode.ConstraintRaw, "A raw constraint was violated"],
  [LangErrorCode.ConstraintOwner, "An owner constraint was violated"],
  [
    LangErrorCode.ConstraintRentExempt,
    "A rent exemption constraint was violated",
  ],
  [LangErrorCode.ConstraintSeeds, "A seeds constraint was violated"],
  [LangErrorCode.ConstraintExecutable, "An executable constraint was violated"],
  [
    LangErrorCode.ConstraintState,
    "Deprecated Error, feel free to replace with something else",
  ],
  [LangErrorCode.ConstraintAssociated, "An associated constraint was violated"],
  [
    LangErrorCode.ConstraintAssociatedInit,
    "An associated init constraint was violated",
  ],
  [LangErrorCode.ConstraintClose, "A close constraint was violated"],
  [LangErrorCode.ConstraintAddress, "An address constraint was violated"],
  [LangErrorCode.ConstraintZero, "Expected zero account discriminant"],
  [LangErrorCode.ConstraintTokenMint, "A token mint constraint was violated"],
  [LangErrorCode.ConstraintTokenOwner, "A token owner constraint was violated"],
  [
    LangErrorCode.ConstraintMintMintAuthority,
    "A mint mint authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintFreezeAuthority,
    "A mint freeze authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintDecimals,
    "A mint decimals constraint was violated",
  ],
  [LangErrorCode.ConstraintSpace, "A space constraint was violated"],
  [
    LangErrorCode.ConstraintAccountIsNone,
    "A required account for the constraint is None",
  ],
  [
    LangErrorCode.ConstraintTokenTokenProgram,
    "A token account token program constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintTokenProgram,
    "A mint token program constraint was violated",
  ],
  [
    LangErrorCode.ConstraintAssociatedTokenTokenProgram,
    "An associated token account token program constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintGroupPointerExtension,
    "A group pointer extension constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintGroupPointerExtensionAuthority,
    "A group pointer extension authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintGroupPointerExtensionGroupAddress,
    "A group pointer extension group address constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintGroupMemberPointerExtension,
    "A group member pointer extension constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintGroupMemberPointerExtensionAuthority,
    "A group member pointer extension authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintGroupMemberPointerExtensionMemberAddress,
    "A group member pointer extension group address constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintMetadataPointerExtension,
    "A metadata pointer extension constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintMetadataPointerExtensionAuthority,
    "A metadata pointer extension authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintMetadataPointerExtensionMetadataAddress,
    "A metadata pointer extension metadata address constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintCloseAuthorityExtension,
    "A close authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintCloseAuthorityExtensionAuthority,
    "A close authority extension authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintPermanentDelegateExtension,
    "A permanent delegate extension constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintPermanentDelegateExtensionDelegate,
    "A permanent delegate extension delegate constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintTransferHookExtension,
    "A transfer hook extension constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintTransferHookExtensionAuthority,
    "A transfer hook extension authority constraint was violated",
  ],
  [
    LangErrorCode.ConstraintMintTransferHookExtensionProgramId,
    "A transfer hook extension transfer hook program id constraint was violated",
  ],

  // Require.
  [LangErrorCode.RequireViolated, "A require expression was violated"],
  [LangErrorCode.RequireEqViolated, "A require_eq expression was violated"],
  [
    LangErrorCode.RequireKeysEqViolated,
    "A require_keys_eq expression was violated",
  ],
  [LangErrorCode.RequireNeqViolated, "A require_neq expression was violated"],
  [
    LangErrorCode.RequireKeysNeqViolated,
    "A require_keys_neq expression was violated",
  ],
  [LangErrorCode.RequireGtViolated, "A require_gt expression was violated"],
  [LangErrorCode.RequireGteViolated, "A require_gte expression was violated"],

  // Accounts.
  [
    LangErrorCode.AccountDiscriminatorAlreadySet,
    "The account discriminator was already set on this account",
  ],
  [
    LangErrorCode.AccountDiscriminatorNotFound,
    "No discriminator was found on the account",
  ],
  [
    LangErrorCode.AccountDiscriminatorMismatch,
    "Account discriminator did not match what was expected",
  ],
  [LangErrorCode.AccountDidNotDeserialize, "Failed to deserialize the account"],
  [LangErrorCode.AccountDidNotSerialize, "Failed to serialize the account"],
  [
    LangErrorCode.AccountNotEnoughKeys,
    "Not enough account keys given to the instruction",
  ],
  [LangErrorCode.AccountNotMutable, "The given account is not mutable"],
  [
    LangErrorCode.AccountOwnedByWrongProgram,
    "The given account is owned by a different program than expected",
  ],
  [LangErrorCode.InvalidProgramId, "Program ID was not as expected"],
  [LangErrorCode.InvalidProgramExecutable, "Program account is not executable"],
  [LangErrorCode.AccountNotSigner, "The given account did not sign"],
  [
    LangErrorCode.AccountNotSystemOwned,
    "The given account is not owned by the system program",
  ],
  [
    LangErrorCode.AccountNotInitialized,
    "The program expected this account to be already initialized",
  ],
  [
    LangErrorCode.AccountNotProgramData,
    "The given account is not a program data account",
  ],
  [
    LangErrorCode.AccountNotAssociatedTokenAccount,
    "The given account is not the associated token account",
  ],
  [
    LangErrorCode.AccountSysvarMismatch,
    "The given public key does not match the required sysvar",
  ],
  [
    LangErrorCode.AccountReallocExceedsLimit,
    "The account reallocation exceeds the MAX_PERMITTED_DATA_INCREASE limit",
  ],
  [
    LangErrorCode.AccountDuplicateReallocs,
    "The account was duplicated for more than one reallocation",
  ],

  // Miscellaneous
  [
    LangErrorCode.DeclaredProgramIdMismatch,
    "The declared program id does not match the actual program id",
  ],
  [
    LangErrorCode.TryingToInitPayerAsProgramAccount,
    "You cannot/should not initialize the payer account as a program account",
  ],
  [
    LangErrorCode.InvalidNumericConversion,
    "The program could not perform the numeric conversion, out of range integral type conversion attempted",
  ],

  // Deprecated
  [
    LangErrorCode.Deprecated,
    "The API being used is deprecated and should no longer be used",
  ],
]);
