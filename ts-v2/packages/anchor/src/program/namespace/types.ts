import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import {
  Idl,
  IdlInstructionAccounts as IdlInstructionAccounts,
  IdlInstructionAccountItem,
  IdlField,
  IdlInstruction,
  IdlType,
  IdlTypeDef,
  IdlTypeDefTyEnum,
  IdlTypeDefTyStruct,
  IdlTypeDefTyType,
  IdlDefinedFields,
  IdlDefinedFieldsNamed,
  IdlDefinedFieldsTuple,
  IdlArrayLen,
} from "../../idl";
import { Accounts, Context } from "../context";
import { MethodsBuilder } from "./methods";

/**
 * All instructions for an IDL.
 */
export type AllInstructions<I extends Idl> = I["instructions"][number];

/**
 * Returns a type of instruction name to the IdlInstruction.
 */
type InstructionMap<I extends IdlInstruction> = {
  [K in I["name"]]: I & { name: K };
};

/**
 * Returns a type of instruction name to the IdlInstruction.
 */
export type AllInstructionsMap<I extends Idl> = InstructionMap<
  AllInstructions<I>
>;

/**
 * All accounts for an IDL.
 */
export type AllAccounts<I extends Idl> = ResolveIdlTypePointer<I, "accounts">;

/**
 * Returns a type of instruction name to the IdlInstruction.
 */
type AccountMap<I extends IdlTypeDef[]> = {
  [K in I[number]["name"]]: I & { name: K };
};

/**
 * Returns a type of instruction name to the IdlInstruction.
 */
export type AllAccountsMap<I extends Idl> = AccountMap<AllAccounts<I>>;

/**
 * All events for an IDL.
 */
export type AllEvents<I extends Idl> = ResolveIdlTypePointer<I, "events">;

export type MakeInstructionsNamespace<
  IDL extends Idl,
  I extends IdlInstruction,
  Ret,
  Mk extends { [M in keyof InstructionMap<I>]: unknown } = {
    [M in keyof InstructionMap<I>]: unknown;
  }
> = {
  [M in keyof InstructionMap<I>]: InstructionContextFn<
    IDL,
    InstructionMap<I>[M],
    Ret
  > &
    Mk[M];
};

export type MakeMethodsNamespace<IDL extends Idl, I extends IdlInstruction> = {
  [M in keyof InstructionMap<I>]: MethodsFn<
    IDL,
    InstructionMap<I>[M],
    MethodsBuilder<IDL, InstructionMap<I>[M]>
  >;
};

export type InstructionContextFn<
  IDL extends Idl,
  I extends AllInstructions<IDL>,
  Ret
> = (...args: InstructionContextFnArgs<IDL, I>) => Ret;

export type InstructionContextFnArgs<
  IDL extends Idl,
  I extends IDL["instructions"][number]
> = [
  ...ArgsTuple<I["args"], IdlTypes<IDL>>,
  Context<Accounts<I["accounts"][number]>>
];

export type InstructionAccountAddresses<
  IDL extends Idl,
  I extends AllInstructions<IDL>
> = InstructionAccountsAddresses<I["accounts"][number]>;

type InstructionAccountsAddresses<
  A extends IdlInstructionAccountItem = IdlInstructionAccountItem
> = {
  [N in A["name"]]: InstructionAccountsAddress<A & { name: N }>;
};

type InstructionAccountsAddress<A extends IdlInstructionAccountItem> =
  A extends IdlInstructionAccounts
    ? InstructionAccountsAddresses<A["accounts"][number]>
    : PublicKey;

export type MethodsFn<
  IDL extends Idl,
  I extends IDL["instructions"][number],
  Ret
> = (...args: ArgsTuple<I["args"], IdlTypes<IDL>>) => Ret;

type TypeMap = {
  pubkey: PublicKey;
  bool: boolean;
  string: string;
  bytes: Buffer;
} & {
  [K in "u8" | "i8" | "u16" | "i16" | "u32" | "i32" | "f32" | "f64"]: number;
} & {
  [K in "u64" | "i64" | "u128" | "i128" | "u256" | "i256"]: BN;
};

export type DecodeType<T extends IdlType, Defined> = IdlType extends T
  ? unknown
  : T extends keyof TypeMap
  ? TypeMap[T]
  : T extends { defined: { name: keyof Defined } }
  ? Defined[T["defined"]["name"]]
  : T extends { option: IdlType }
  ? DecodeType<T["option"], Defined> | null
  : T extends { coption: IdlType }
  ? DecodeType<T["coption"], Defined> | null
  : T extends { vec: IdlType }
  ? DecodeType<T["vec"], Defined>[]
  : T extends { array: [defined: IdlType, size: IdlArrayLen] }
  ? DecodeType<T["array"][0], Defined>[]
  : unknown;

/**
 * Tuple of arguments.
 */
type ArgsTuple<A extends IdlField[], Defined> = {
  [K in keyof A]: A[K] extends IdlField
    ? DecodeType<A[K]["type"], Defined>
    : unknown;
} & unknown[];
/**
 * flat {a: number, b: {c: string}} into number | string
 */
type UnboxToUnion<T> = T extends (infer U)[]
  ? UnboxToUnion<U>
  : T extends Record<string, never> // empty object, eg: named enum variant without fields
  ? "__empty_object__"
  : T extends Record<string, infer V> // object with props, eg: struct
  ? UnboxToUnion<V>
  : T;

type DecodeDefinedField<F, Defined> = F extends IdlType
  ? DecodeType<F, Defined>
  : never;

/**
 * decode enum variant: named or tuple
 */
type DecodeDefinedFields<
  F extends IdlDefinedFields,
  Defined
> = F extends IdlDefinedFieldsNamed
  ? {
      [F2 in F[number] as F2["name"]]: DecodeDefinedField<F2["type"], Defined>;
    }
  : F extends IdlDefinedFieldsTuple
  ? {
      [F3 in keyof F as Exclude<F3, keyof unknown[]>]: DecodeDefinedField<
        F[F3],
        Defined
      >;
    }
  : Record<string, never>;

type DecodeEnumVariants<I extends IdlTypeDefTyEnum, Defined> = {
  [V in I["variants"][number] as V["name"]]: DecodeDefinedFields<
    NonNullable<V["fields"]>,
    Defined
  >;
};

type ValueOf<T> = T[keyof T];
type XorEnumVariants<T extends Record<string, unknown>> = ValueOf<{
  [K1 in keyof T]: {
    [K2 in Exclude<keyof T, K1>]?: never;
  } & { [K2 in K1]: T[K2] };
}>;

type DecodeEnum<I extends IdlTypeDefTyEnum, Defined> = XorEnumVariants<
  DecodeEnumVariants<I, Defined>
>;

type DecodeStruct<I extends IdlTypeDefTyStruct, Defined> = DecodeDefinedFields<
  NonNullable<I["fields"]>,
  Defined
>;

type DecodeAlias<I extends IdlTypeDefTyType, Defined> = DecodeType<
  I["alias"],
  Defined
>;

export type TypeDef<
  I extends IdlTypeDef,
  Defined
> = I["type"] extends IdlTypeDefTyEnum
  ? DecodeEnum<I["type"], Defined>
  : I["type"] extends IdlTypeDefTyStruct
  ? DecodeStruct<I["type"], Defined>
  : I["type"] extends IdlTypeDefTyType
  ? DecodeAlias<I["type"], Defined>
  : never;

type TypeDefDictionary<T extends IdlTypeDef[], Defined> = {
  [K in T[number] as K["name"]]: TypeDef<K, Defined>;
};

type DecodedHelper<T extends IdlTypeDef[], Defined> = {
  [D in T[number] as D["name"]]: TypeDef<D, Defined>;
};

type UnknownType = "__unknown_defined_type__";
/**
 * Empty "defined" object to produce `UnknownType` instead of never/unknown
 * during IDL types decoding.
 */
type EmptyDefined = Record<UnknownType, never>;

type RecursiveDepth2<
  T extends IdlTypeDef[],
  Defined = EmptyDefined,
  Decoded = DecodedHelper<T, Defined>
> = UnknownType extends UnboxToUnion<Decoded>
  ? RecursiveDepth3<T, DecodedHelper<T, Defined>>
  : Decoded;

type RecursiveDepth3<
  T extends IdlTypeDef[],
  Defined = EmptyDefined,
  Decoded = DecodedHelper<T, Defined>
> = UnknownType extends UnboxToUnion<Decoded>
  ? RecursiveDepth4<T, DecodedHelper<T, Defined>>
  : Decoded;

type RecursiveDepth4<
  T extends IdlTypeDef[],
  Defined = EmptyDefined
> = DecodedHelper<T, Defined>;

/**
 * TypeScript can't handle truly recursive type (RecursiveTypes instead of RecursiveDepth2).
 * Hence we're doing recursion of depth=4 manually
 *  */
type RecursiveTypes<
  T extends IdlTypeDef[],
  Defined = EmptyDefined,
  Decoded = DecodedHelper<T, Defined>
> =
  // check if some of decoded types is Unknown (not decoded properly)
  UnknownType extends UnboxToUnion<Decoded>
    ? RecursiveDepth2<T, DecodedHelper<T, Defined>>
    : Decoded;

export type IdlTypes<I extends Idl> = RecursiveTypes<NonNullable<I["types"]>>;

export type IdlErrors<I extends Idl> = NonNullable<I["errors"]>[number];

export type IdlAccounts<I extends Idl> = ResolveIdlPointerSection<
  I,
  "accounts"
>;

export type IdlEvents<I extends Idl> = ResolveIdlPointerSection<I, "events">;

type IdlPointerSection = keyof Pick<Idl, "accounts" | "events">;

type ResolveIdlPointerSection<
  I extends Idl,
  K extends IdlPointerSection,
  T extends ResolveIdlTypePointer<I, K> = ResolveIdlTypePointer<I, K>
> = TypeDefDictionary<T extends [] ? IdlTypeDef[] : T, IdlTypes<I>>;

type ResolveIdlTypePointer<
  I extends Idl,
  Key extends IdlPointerSection
> = FilterTuple<
  NonNullable<I["types"]>,
  { name: NonNullable<I[Key]>[number]["name"] }
>;

type FilterTuple<T extends unknown[], F> = T extends [infer Head, ...infer Tail]
  ? [Head] extends [F]
    ? [Head, ...FilterTuple<Tail, F>]
    : FilterTuple<Tail, F>
  : [];
