import { Buffer } from "buffer";
import * as base64 from "../../utils/bytes/base64.js";
import { Idl, IdlDiscriminator } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { IdlCodec } from "./codecs.js";
import { EventCoder } from "../index.js";

export class BorshEventCoder implements EventCoder {
  /**
   * Maps event type identifier to a codec.
   */
  private codecs: Map<
    string,
    { discriminator: IdlDiscriminator; codec: IdlCodec }
  >;

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
        {
          discriminator: ev.discriminator,
          codec: IdlCoder.typeDefCodec({ typeDef, types }),
        },
      ] as const;
    });
    this.codecs = new Map(codecs);
  }

  public decode(log: string): {
    name: string;
    data: any;
  } | null {
    let logArr: Buffer;
    // This will throw if log length is not a multiple of 4.
    try {
      logArr = base64.decode(log);
    } catch (e) {
      return null;
    }

    for (const [name, entry] of this.codecs) {
      const givenDisc = logArr.subarray(0, entry.discriminator.length);
      const matches = givenDisc.equals(Buffer.from(entry.discriminator));
      if (matches) {
        return {
          name,
          data: entry.codec.decode(logArr.subarray(givenDisc.length)),
        };
      }
    }

    return null;
  }
}
