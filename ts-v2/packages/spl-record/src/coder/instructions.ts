// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplRecordInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "initialize": {
        return encodeInitialize(ix);
      }
      case "write": {
        return encodeWrite(ix);
      }
      case "setAuthority": {
        return encodeSetAuthority(ix);
      }
      case "closeAccount": {
        return encodeCloseAccount(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplRecord does not have state");
  }
}

function encodeInitialize({}: any): Buffer {
  return encodeData({ initialize: {} }, 1);
}

function encodeWrite({ offset, data }: any): Buffer {
  return encodeData({ write: { offset, data } }, 1 + 8 + 4 + data.length);
}

function encodeSetAuthority({}: any): Buffer {
  return encodeData({ setAuthority: {} }, 1);
}

function encodeCloseAccount({}: any): Buffer {
  return encodeData({ closeAccount: {} }, 1);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(0, B.struct([]), "initialize");
LAYOUT.addVariant(1, B.struct([B.u64("offset"), B.bytes("data")]), "write");
LAYOUT.addVariant(2, B.struct([]), "setAuthority");
LAYOUT.addVariant(3, B.struct([]), "closeAccount");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
