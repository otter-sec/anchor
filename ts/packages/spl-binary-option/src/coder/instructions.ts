// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplBinaryOptionInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "initializeBinaryOption": {
        return encodeInitializeBinaryOption(ix);
      }
      case "trade": {
        return encodeTrade(ix);
      }
      case "settle": {
        return encodeSettle(ix);
      }
      case "collect": {
        return encodeCollect(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplBinaryOption does not have state");
  }
}

function encodeInitializeBinaryOption({ decimals }: any): Buffer {
  return encodeData({ initializeBinaryOption: { decimals } }, 1 + 1);
}

function encodeTrade({ size, buyPrice, sellPrice }: any): Buffer {
  return encodeData({ trade: { size, buyPrice, sellPrice } }, 1 + 8 + 8 + 8);
}

function encodeSettle({}: any): Buffer {
  return encodeData({ settle: {} }, 1);
}

function encodeCollect({}: any): Buffer {
  return encodeData({ collect: {} }, 1);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(0, B.struct([B.u8("decimals")]), "initializeBinaryOption");
LAYOUT.addVariant(
  1,
  B.struct([B.u64("size"), B.u64("buyPrice"), B.u64("sellPrice")]),
  "trade"
);
LAYOUT.addVariant(2, B.struct([]), "settle");
LAYOUT.addVariant(3, B.struct([]), "collect");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
