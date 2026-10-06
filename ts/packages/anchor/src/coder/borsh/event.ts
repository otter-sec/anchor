import { getBase64Encoder, ReadonlyUint8Array } from "@solana/kit";
import { Idl } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { DiscriminatedIdlCodec, getDiscriminatedIdlCodec } from "./codecs.js";
import { EventCoder } from "../index.js";

export class BorshEventCoder implements EventCoder {
  /**
   * Maps event type identifier to a codec.
   */
  private codecs: Map<string, DiscriminatedIdlCodec>;

  public constructor(idl: Idl) {
    if (!idl.events) {
      this.codecs = new Map();
      return;
    }

    const types = idl.types;
    if (!types) {
      throw new Error("Events require `idl.types`");
    }

    const codecs = idl.events.map((ev) => {
      const typeDef = types.find((ty) => ty.name === ev.name);
      if (!typeDef) {
        throw new Error(`Event not found: ${ev.name}`);
      }
      return [
        ev.name,
        getDiscriminatedIdlCodec(
          ev.discriminator,
          IdlCoder.typeDefCodec({ typeDef, types })
        ),
      ] as const;
    });
    this.codecs = new Map(codecs);
  }

  public decode(log: string): {
    name: string;
    data: any;
  } | null {
    let logArr: ReadonlyUint8Array;
    // This will throw if the log is not valid base64.
    try {
      logArr = getBase64Encoder().encode(log);
    } catch (e) {
      return null;
    }

    for (const [name, codec] of this.codecs) {
      if (codec.matches(logArr)) {
        return { name, data: codec.decode(logArr) };
      }
    }

    return null;
  }
}
