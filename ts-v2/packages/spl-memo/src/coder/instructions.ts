// @ts-nocheck
import { Idl, InstructionCoder } from "@anchor-lang/core";

export class SplMemoInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "addMemo": {
        return encodeAddMemo(ix);
      }

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("SplMemo does not have state");
  }
}

function encodeAddMemo({ memo }: any): Buffer {
  return Buffer.from(memo);
}
