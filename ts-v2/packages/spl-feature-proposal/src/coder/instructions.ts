// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplFeatureProposalInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "propose": {
        return encodePropose(ix);
      }
      case "tally": {
        return encodeTally(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplFeatureProposal does not have state");
  }
}

function encodePropose({ tokensToMint, acceptanceCriteria }: any): Buffer {
  return encodeData(
    { propose: { tokensToMint, acceptanceCriteria } },
    1 + 8 + 8 + 8
  );
}

function encodeTally({}: any): Buffer {
  return encodeData({ tally: {} }, 1);
}

const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(
  0,
  B.struct([
    B.u64("tokensToMint"),
    B.struct(
      [B.u64("tokensRequired"), B.i64("deadline")],
      "acceptanceCriteria"
    ),
  ]),
  "propose"
);
LAYOUT.addVariant(1, B.struct([]), "tally");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
