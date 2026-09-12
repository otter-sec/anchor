import camelCase from "camelcase";
import { Buffer } from "buffer";
import { PublicKey } from "@solana/web3.js";
import * as borsh from "@anchor-lang/borsh";

export type Idl = {
  address: string;
  metadata: IdlMetadata;
  docs?: string[];
  instructions: IdlInstruction[];
  accounts?: IdlAccount[];
  events?: IdlEvent[];
  errors?: IdlErrorCode[];
  types?: IdlTypeDef[];
  constants?: IdlConst[];
};

export type IdlMetadata = {
  name: string;
  version: string;
  spec: string;
  description?: string;
  repository?: string;
  dependencies?: IdlDependency[];
  contact?: string;
  deployments?: IdlDeployments;
};

export type IdlDependency = {
  name: string;
  version: string;
};

export type IdlDeployments = {
  mainnet?: string;
  testnet?: string;
  devnet?: string;
  localnet?: string;
};

export type IdlInstruction = {
  name: string;
  docs?: string[];
  discriminator: IdlDiscriminator;
  accounts: IdlInstructionAccountItem[];
  args: IdlField[];
  returns?: IdlType;
};

export type IdlInstructionAccountItem =
  | IdlInstructionAccount
  | IdlInstructionAccounts;

export type IdlInstructionAccount = {
  name: string;
  docs?: string[];
  writable?: boolean;
  signer?: boolean;
  optional?: boolean;
  address?: string;
  pda?: IdlPda;
  relations?: string[];
};

export type IdlInstructionAccounts = {
  name: string;
  accounts: IdlInstructionAccount[];
};

export type IdlPda = {
  seeds: IdlSeed[];
  program?: IdlSeed;
};

export type IdlSeed = IdlSeedConst | IdlSeedArg | IdlSeedAccount;

export type IdlSeedConst = {
  kind: "const";
  value: number[];
};

export type IdlSeedArg = {
  kind: "arg";
  path: string;
};

export type IdlSeedAccount = {
  kind: "account";
  path: string;
  account?: string;
};

export type IdlAccount = {
  name: string;
  discriminator: IdlDiscriminator;
};

export type IdlEvent = {
  name: string;
  discriminator: IdlDiscriminator;
};

export type IdlConst = {
  name: string;
  type: IdlType;
  value: string;
};

export type IdlErrorCode = {
  name: string;
  code: number;
  msg?: string;
};

export type IdlField = {
  name: string;
  docs?: string[];
  type: IdlType;
};

export type IdlTypeDef = {
  name: string;
  docs?: string[];
  serialization?: IdlSerialization;
  repr?: IdlRepr;
  generics?: IdlTypeDefGeneric[];
  type: IdlTypeDefTy;
};

export type IdlSerialization =
  | "borsh"
  | "bytemuck"
  | "bytemuckunsafe"
  | { custom: string };

export type IdlRepr = IdlReprRust | IdlReprC | IdlReprTransparent;

export type IdlReprRust = {
  kind: "rust";
} & IdlReprModifier;

export type IdlReprC = {
  kind: "c";
} & IdlReprModifier;

export type IdlReprTransparent = {
  kind: "transparent";
};

export type IdlReprModifier = {
  packed?: boolean;
  align?: number;
};

export type IdlTypeDefGeneric = IdlTypeDefGenericType | IdlTypeDefGenericConst;

export type IdlTypeDefGenericType = {
  kind: "type";
  name: string;
};

export type IdlTypeDefGenericConst = {
  kind: "const";
  name: string;
  type: string;
};

export type IdlTypeDefTy =
  | IdlTypeDefTyEnum
  | IdlTypeDefTyStruct
  | IdlTypeDefTyType;

export type IdlTypeDefTyStruct = {
  kind: "struct";
  fields?: IdlDefinedFields;
};

export type IdlTypeDefTyEnum = {
  kind: "enum";
  variants: IdlEnumVariant[];
};

export type IdlTypeDefTyType = {
  kind: "type";
  alias: IdlType;
};

export type IdlEnumVariant = {
  name: string;
  fields?: IdlDefinedFields;
};

export type IdlDefinedFields = IdlDefinedFieldsNamed | IdlDefinedFieldsTuple;

export type IdlDefinedFieldsNamed = IdlField[];

export type IdlDefinedFieldsTuple = IdlType[];

export type IdlArrayLen = IdlArrayLenGeneric | IdlArrayLenValue;

export type IdlArrayLenGeneric = {
  generic: string;
};

export type IdlArrayLenValue = number;

export type IdlGenericArg = IdlGenericArgType | IdlGenericArgConst;

export type IdlGenericArgType = { kind: "type"; type: IdlType };

export type IdlGenericArgConst = { kind: "const"; value: string };

export type IdlType =
  | "bool"
  | "u8"
  | "i8"
  | "u16"
  | "i16"
  | "u32"
  | "i32"
  | "f32"
  | "u64"
  | "i64"
  | "f64"
  | "u128"
  | "i128"
  | "u256"
  | "i256"
  | "bytes"
  | "string"
  | "pubkey"
  | IdlTypeOption
  | IdlTypeCOption
  | IdlTypeVec
  | IdlTypeArray
  | IdlTypeDefined
  | IdlTypeGeneric;

export type IdlTypeOption = {
  option: IdlType;
};

export type IdlTypeCOption = {
  coption: IdlType;
};

export type IdlTypeVec = {
  vec: IdlType;
};

export type IdlTypeArray = {
  array: [idlType: IdlType, size: IdlArrayLen];
};

export type IdlTypeDefined = {
  defined: {
    name: string;
    generics?: IdlGenericArg[];
  };
};

export type IdlTypeGeneric = {
  generic: string;
};

export type IdlDiscriminator = number[];

export function isCompositeAccounts(
  accountItem: IdlInstructionAccountItem
): accountItem is IdlInstructionAccounts {
  return "accounts" in accountItem;
}

// Deterministic IDL address as a function of the program id.
export async function idlAddress(programId: PublicKey): Promise<PublicKey> {
  const base = (await PublicKey.findProgramAddress([], programId))[0];
  return await PublicKey.createWithSeed(base, seed(), programId);
}

// Seed for generating the idlAddress.
export function seed(): string {
  return "anchor:idl";
}

// The on-chain account of the IDL.
export interface IdlProgramAccount {
  authority: PublicKey;
  data: Buffer;
}

const IDL_ACCOUNT_LAYOUT: borsh.Layout<IdlProgramAccount> = borsh.struct([
  borsh.publicKey("authority"),
  borsh.vecU8("data"),
]);

export function decodeIdlAccount(data: Buffer): IdlProgramAccount {
  return IDL_ACCOUNT_LAYOUT.decode(data);
}

export function encodeIdlAccount(acc: IdlProgramAccount): Buffer {
  const buffer = Buffer.alloc(1000); // TODO: use a tighter buffer.
  const len = IDL_ACCOUNT_LAYOUT.encode(acc, buffer);
  return buffer.slice(0, len);
}

/**
 * Convert the given IDL to camelCase.
 *
 * The IDL is generated from Rust which has different conventions compared to
 * JS/TS, e.g. instruction names in Rust are snake_case.
 *
 * The conversion happens automatically for programs, however, if you are using
 * internals such as `BorshInstructionCoder` and you only have the original
 * (not camelCase) IDL, you might need to use this function.
 *
 * @param idl IDL to convert to camelCase
 * @returns camelCase version of the IDL
 */
export function convertIdlToCamelCase<I extends Idl>(idl: I) {
  const KEYS_TO_CONVERT = ["name", "path", "account", "relations", "generic"];

  // `my_account.field` is getting converted to `myAccountField` but we
  // need `myAccount.field`.
  const toCamelCase = (s: any) =>
    s
      .split(".")
      .map((part: any) => camelCase(part, { locale: false }))
      .join(".");

  const recursivelyConvertNamesToCamelCase = (obj: Record<string, any>) => {
    for (const key in obj) {
      const val = obj[key];
      if (KEYS_TO_CONVERT.includes(key)) {
        obj[key] = Array.isArray(val) ? val.map(toCamelCase) : toCamelCase(val);
      } else if (typeof val === "object") {
        recursivelyConvertNamesToCamelCase(val);
      }
    }
  };

  const camelCasedIdl = structuredClone(idl);
  recursivelyConvertNamesToCamelCase(camelCasedIdl);
  return camelCasedIdl;
}

/** Conveniently handle all defined field kinds with proper type support. */
export function handleDefinedFields<U, N, T>(
  fields: IdlDefinedFields | undefined,
  unitCb: () => U,
  namedCb: (fields: IdlDefinedFieldsNamed) => N,
  tupleCb: (fields: IdlDefinedFieldsTuple) => T
) {
  // Unit
  if (!fields?.length) return unitCb();

  // Named
  if ((fields as IdlDefinedFieldsNamed)[0].name) {
    return namedCb(fields as IdlDefinedFieldsNamed);
  }

  // Tuple
  return tupleCb(fields as IdlDefinedFieldsTuple);
}
