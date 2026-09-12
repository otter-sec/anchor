// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplNameServiceInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "create": {
        return encodeCreate(ix);
      }
      case "update": {
        return encodeUpdate(ix);
      }
      case "transfer": {
        return encodeTransfer(ix);
      }
      case "delete": {
        return encodeDelete(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplNameService does not have state");
  }
}

function encodeCreate({ hashedName, lamports, space }: any): Buffer {
  return encodeData(
    { create: { hashedName, lamports, space } },
    1 + 4 + hashedName.length + 8 + 4
  );
}

function encodeUpdate({ offset, data }: any): Buffer {
  return encodeData({ update: { offset, data } }, 1 + 4 + 4 + data.length);
}

function encodeTransfer({ newOwner }: any): Buffer {
  return encodeData({ transfer: { newOwner } }, 1 + 32);
}

function encodeDelete({}: any): Buffer {
  return encodeData({ delete: {} }, 1);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(
  0,
  B.struct([B.bytes("hashedName"), B.u64("lamports"), B.u32("space")]),
  "create"
);
LAYOUT.addVariant(1, B.struct([B.u32("offset"), B.bytes("data")]), "update");
LAYOUT.addVariant(2, B.struct([B.publicKey("newOwner")]), "transfer");
LAYOUT.addVariant(3, B.struct([]), "delete");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
