// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplAssociatedTokenAccountInstructionCoder
  implements InstructionCoder
{
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "create": {
        return encodeCreate(ix);
      }
      case "createIdempotent": {
        return encodeCreateIdempotent(ix);
      }
      case "recoverNested": {
        return encodeRecoverNested(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplAssociatedTokenAccount does not have state");
  }
}

function encodeCreate({}: any): Buffer {
  return encodeData({ create: {} }, 1);
}

function encodeCreateIdempotent({}: any): Buffer {
  return encodeData({ createIdempotent: {} }, 1);
}

function encodeRecoverNested({}: any): Buffer {
  return encodeData({ recoverNested: {} }, 1);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(0, B.struct([]), "create");
LAYOUT.addVariant(1, B.struct([]), "createIdempotent");
LAYOUT.addVariant(2, B.struct([]), "recoverNested");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
