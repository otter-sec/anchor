import { Buffer } from "buffer";
import { Layout } from "buffer-layout";
import { Idl } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { TypesCoder } from "../index.js";

/**
 * Encodes and decodes user-defined types.
 */
export class BorshTypesCoder<N extends string = string> implements TypesCoder {
  /**
   * Maps type name to a layout.
   */
  private typeLayouts: Map<N, Layout>;

  public constructor(idl: Idl) {
    const types = idl.types;
    if (!types) {
      this.typeLayouts = new Map();
      return;
    }

    const layouts: [N, Layout][] = types
      .filter((ty) => !ty.generics)
      .map((ty) => [
        ty.name as N,
        IdlCoder.typeDefLayout({ typeDef: ty, types }),
      ]);
    this.typeLayouts = new Map(layouts);
  }

  public encode<T = any>(name: N, type: T): Buffer {
    const buffer = Buffer.alloc(1000); // TODO: use a tighter buffer.
    const layout = this.typeLayouts.get(name);
    if (!layout) {
      throw new Error(`Unknown type: ${name}`);
    }
    const len = layout.encode(type, buffer);

    return buffer.slice(0, len);
  }

  public decode<T = any>(name: N, data: Buffer): T {
    const layout = this.typeLayouts.get(name);
    if (!layout) {
      throw new Error(`Unknown type: ${name}`);
    }
    return layout.decode(data);
  }
}
