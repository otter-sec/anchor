// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplStatelessAsksInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "acceptOffer": {
        return encodeAcceptOffer(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplStatelessAsks does not have state");
  }
}

function encodeAcceptOffer({
  hasMetadata,
  makerSize,
  takerSize,
  bumpSeed,
}: any): Buffer {
  return encodeData(
    { acceptOffer: { hasMetadata, makerSize, takerSize, bumpSeed } },
    1 + 1 + 8 + 8 + 1
  );
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(
  0,
  B.struct([
    B.bool("hasMetadata"),
    B.u64("makerSize"),
    B.u64("takerSize"),
    B.u8("bumpSeed"),
  ]),
  "acceptOffer"
);

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
