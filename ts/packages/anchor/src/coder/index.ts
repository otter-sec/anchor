import { ReadonlyUint8Array } from "@solana/kit";
import { IdlEvent } from "../idl.js";
import { Event } from "../program/event.js";

export * from "./borsh/index.js";

/**
 * Coder provides a facade for encoding and decoding all IDL related objects.
 */
export interface Coder<A extends string = string, T extends string = string> {
  /**
   * Instruction coder.
   */
  readonly instruction: InstructionCoder;

  /**
   * Account coder.
   */
  readonly accounts: AccountsCoder<A>;

  /**
   * Coder for events.
   */
  readonly events: EventCoder;

  /**
   * Coder for user-defined types.
   */
  readonly types: TypesCoder<T>;
}

export interface AccountsCoder<A extends string = string> {
  encode<T = any>(accountName: A, account: T): Promise<ReadonlyUint8Array>;
  decode<T = any>(accountName: A, data: ReadonlyUint8Array): T;
  decodeUnchecked<T = any>(accountName: A, data: ReadonlyUint8Array): T;
  memcmp(accountName: A, appendData?: ReadonlyUint8Array): any;
  size(accountName: A): number;
}

export interface InstructionCoder {
  encode(ixName: string, ix: any): ReadonlyUint8Array;
}

export interface EventCoder {
  decode<E extends IdlEvent = IdlEvent, T = Record<string, string>>(
    log: string
  ): Event<E, T> | null;
}

export interface TypesCoder<N extends string = string> {
  encode<T = any>(typeName: N, type: T): ReadonlyUint8Array;
  decode<T = any>(typeName: N, typeData: ReadonlyUint8Array): T;
}
