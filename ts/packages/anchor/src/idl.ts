import { bs58, utf8 } from "./utils/bytes/index.js";
import { inflate, ungzip } from "pako";
import { toCamelCase } from "./utils/case.js";
import { Buffer } from "buffer";
import { PublicKey } from "@solana/web3.js";

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

// Account format defined at
// https://github.com/solana-program/program-metadata/blob/734e947d/clients/js/src/generated/accounts/metadata.ts#L123-L138
const PROGRAM_METADATA_PROGRAM_ID = new PublicKey(
  "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S"
);
const IDL_METADATA_SEED = "idl";
const ACCOUNT_DISCRIMINATOR_METADATA = 2;
const DATA_SOURCE_DIRECT = 0;
// Only JSON formatted data is currently supported
export const FORMAT_JSON = 1;
const SEED_SIZE = 16;
const DATA_LENGTH_SIZE = 4;
const DATA_LENGTH_PADDING = 5;
const PUBKEY_SIZE = 32;
const ZEROABLE_OPTION_PUBKEY_SIZE = 32;
const METADATA_HEADER_SIZE =
  1 +
  PUBKEY_SIZE +
  ZEROABLE_OPTION_PUBKEY_SIZE +
  1 +
  1 +
  SEED_SIZE +
  1 +
  1 +
  1 +
  1;

export enum MetadataCompression {
  None = 0,
  Gzip = 1,
  Zlib = 2,
}

export enum MetadataEncoding {
  None = 0,
  Utf8 = 1,
  Base58 = 2,
  Base64 = 3,
}

export enum Format {
  None = 0,
  Json = 1,
  Yaml = 2,
  Toml = 3,
}

export type MetadataAccount = {
  format: number;
  dataSource: number;
  compression: MetadataCompression;
  encoding: MetadataEncoding;
  data: Buffer;
};

function encodeMetadataSeed(seed: string): Buffer {
  const encodedSeed = Buffer.from(utf8.encode(seed));
  if (encodedSeed.length > SEED_SIZE) {
    throw new Error(`Metadata seed '${seed}' exceeds ${SEED_SIZE} bytes`);
  }

  const paddedSeed = Buffer.alloc(SEED_SIZE);
  encodedSeed.copy(paddedSeed);
  return paddedSeed;
}

export function idlAddress(programId: PublicKey): PublicKey {
  // Canonical metadata uses a null authority seed, which is serialized as `[]`.
  return PublicKey.findProgramAddressSync(
    [
      programId.toBuffer(),
      Buffer.alloc(0),
      encodeMetadataSeed(IDL_METADATA_SEED),
    ],
    PROGRAM_METADATA_PROGRAM_ID
  )[0];
}

export function seed(): string {
  return "idl";
}

/**
 * Get the parsed IDL from the given account data.
 *
 * **Note:** Only JSON IDLs are supported.
 *
 * @see {@link decodeIdlAccountRaw} to get the raw IDL account.
 *
 * @param data IDL account data
 * @returns the parsed IDL
 */
export function decodeIdlAccount<IDL extends Idl = Idl>(data: Buffer): IDL {
  const { data: rawData, compression, encoding } = decodeIdlAccountRaw(data);
  const decoded = decodeMetadataData(
    uncompressMetadataData(rawData, compression),
    encoding
  );
  return JSON.parse(decoded);
}

/**
 * Decode an IDL account.
 *
 * **Note:** Only JSON IDLs are supported.
 *
 * @see {@link decodeIdlAccount} to get the parsed IDL.
 *
 * @param data IDL account data
 * @returns the decoded account fields
 */
export function decodeIdlAccountRaw(data: Buffer) {
  const minimumSize =
    METADATA_HEADER_SIZE + DATA_LENGTH_SIZE + DATA_LENGTH_PADDING;
  if (data.length < minimumSize) {
    throw new Error("Metadata account is too small");
  }

  let offset = 0;
  const discriminator = data.readUInt8(offset);
  if (discriminator !== ACCOUNT_DISCRIMINATOR_METADATA) {
    throw new Error(
      `Invalid metadata account discriminator: ${discriminator.toString()}`
    );
  }
  offset += 1;

  const program = new PublicKey(data.subarray(offset, offset + PUBKEY_SIZE));
  offset += PUBKEY_SIZE;

  const authorityBytes = data.subarray(
    offset,
    offset + ZEROABLE_OPTION_PUBKEY_SIZE
  );
  const authority = authorityBytes.every((b) => b === 0)
    ? null
    : new PublicKey(authorityBytes);
  offset += ZEROABLE_OPTION_PUBKEY_SIZE;

  const mutable = Boolean(data.readUInt8(offset));
  offset += 1;

  const canonical = Boolean(data.readUInt8(offset));
  offset += 1;

  const seed = utf8.decode(data.subarray(offset, offset + SEED_SIZE));
  offset += SEED_SIZE;

  const encoding = data.readUInt8(offset) as MetadataEncoding;
  offset += 1;

  const compression = data.readUInt8(offset) as MetadataCompression;
  offset += 1;

  const format = data.readUInt8(offset) as Format;
  if (format !== Format.Json) {
    throw new Error(
      `IDL has data format '${format}', only JSON IDLs (${Format.Json}) are supported`
    );
  }
  offset += 1;

  const dataSource = data.readUInt8(offset);
  if (dataSource !== DATA_SOURCE_DIRECT) {
    throw new Error(
      `IDL has source '${dataSource}', only directly embedded data (${DATA_SOURCE_DIRECT}) is supported`
    );
  }
  offset += 1;

  const dataLength = data.readUInt32LE(offset);
  offset += DATA_LENGTH_SIZE + DATA_LENGTH_PADDING;
  if (data.length < offset + dataLength) {
    throw new Error("Metadata account data is truncated");
  }

  return {
    discriminator,
    program,
    authority,
    mutable,
    canonical,
    seed,
    encoding,
    compression,
    format,
    dataSource,
    dataLength,
    data: data.subarray(offset, offset + dataLength),
  };
}

export function uncompressMetadataData(
  data: Buffer,
  compression: MetadataCompression
): Buffer {
  switch (compression) {
    case MetadataCompression.None:
      return data;
    case MetadataCompression.Gzip:
      return Buffer.from(ungzip(data));
    case MetadataCompression.Zlib:
      return Buffer.from(inflate(data));
    default:
      throw new Error(
        `Unsupported metadata compression: ${String(compression as number)}`
      );
  }
}

export function decodeMetadataData(
  data: Buffer,
  encoding: MetadataEncoding
): string {
  switch (encoding) {
    // 'None' is actually hex-encoded
    case MetadataEncoding.None:
      return data.toString("hex");
    case MetadataEncoding.Utf8:
      return utf8.decode(data);
    case MetadataEncoding.Base58:
      return bs58.encode(data);
    case MetadataEncoding.Base64:
      return data.toString("base64");
    default:
      throw new Error(
        `Unsupported metadata encoding: ${String(encoding as number)}`
      );
  }
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
  // need `myAccount.field`, so camelCase each dot-separated segment in
  // isolation. The local helper has a distinct name from the imported
  // `toCamelCase` to avoid the shadowing that would otherwise turn the
  // self-reference below into infinite recursion.
  const toCamelCasePath = (s: any) =>
    s
      .split(".")
      .map((part: any) => toCamelCase(part))
      .join(".");

  const recursivelyConvertNamesToCamelCase = (obj: Record<string, any>) => {
    for (const key in obj) {
      const val = obj[key];
      if (KEYS_TO_CONVERT.includes(key)) {
        obj[key] = Array.isArray(val)
          ? val.map(toCamelCasePath)
          : toCamelCasePath(val);
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
