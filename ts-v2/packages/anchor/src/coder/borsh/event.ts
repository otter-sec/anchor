import { Buffer } from "buffer";
import { Layout } from "buffer-layout";
import * as base64 from "../../utils/bytes/base64.js";
import { Idl, IdlDiscriminator } from "../../idl.js";
import { IdlCoder } from "./idl.js";
import { EventCoder } from "../index.js";

export class BorshEventCoder implements EventCoder {
  /**
   * Maps account type identifier to a layout.
   */
  private layouts: Map<
    string,
    { discriminator: IdlDiscriminator; layout: Layout }
  >;

  public constructor(idl: Idl) {
    if (!idl.events) {
      this.layouts = new Map();
      return;
    }

    const types = idl.types;
    if (!types) {
      throw new Error("Events require `idl.types`");
    }

    const layouts = idl.events.map((ev) => {
      const typeDef = types.find((ty) => ty.name === ev.name);
      if (!typeDef) {
        throw new Error(`Event not found: ${ev.name}`);
      }
      return [
        ev.name,
        {
          discriminator: ev.discriminator,
          layout: IdlCoder.typeDefLayout({ typeDef, types }),
        },
      ] as const;
    });
    this.layouts = new Map(layouts);
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

    for (const [name, layout] of this.layouts) {
      const givenDisc = logArr.subarray(0, layout.discriminator.length);
      const matches = givenDisc.equals(Buffer.from(layout.discriminator));
      if (matches) {
        return {
          name,
          data: layout.layout.decode(logArr.subarray(givenDisc.length)),
        };
      }
    }

    return null;
  }
}
