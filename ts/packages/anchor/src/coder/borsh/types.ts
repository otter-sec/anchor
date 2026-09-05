import { Buffer } from "buffer";
import { Idl } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { IdlCodec } from "./codecs.js";
import { TypesCoder } from "../index.js";

/**
 * Encodes and decodes user-defined types.
 */
export class BorshTypesCoder<N extends string = string> implements TypesCoder {
  /**
   * Maps type name to a codec.
   */
  private typeCodecs: Map<N, IdlCodec>;

  public constructor(idl: Idl) {
    const types = idl.types;
    if (!types) {
      this.typeCodecs = new Map();
      return;
    }

    const codecs: [N, IdlCodec][] = types
      .filter((ty) => !ty.generics)
      .map((ty) => [
        ty.name as N,
        IdlCoder.typeDefCodec({ typeDef: ty, types }),
      ]);
    this.typeCodecs = new Map(codecs);
  }

  public encode<T = any>(name: N, type: T): Buffer {
    const codec = this.typeCodecs.get(name);
    if (!codec) {
      throw new Error(`Unknown type: ${name}`);
    }
    return Buffer.from(codec.encode(type) as Uint8Array);
  }

  public decode<T = any>(name: N, data: Buffer): T {
    const codec = this.typeCodecs.get(name);
    if (!codec) {
      throw new Error(`Unknown type: ${name}`);
    }
    return codec.decode(data) as T;
  }
}
