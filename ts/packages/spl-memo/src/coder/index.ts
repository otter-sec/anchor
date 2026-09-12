import { Idl, Coder } from "@anchor-lang/core";

import { SplMemoAccountsCoder } from "./accounts";
import { SplMemoEventsCoder } from "./events";
import { SplMemoInstructionCoder } from "./instructions";
import { SplMemoTypesCoder } from "./types";

/**
 * Coder for SplMemo
 */
export class SplMemoCoder implements Coder {
  readonly accounts: SplMemoAccountsCoder;
  readonly events: SplMemoEventsCoder;
  readonly instruction: SplMemoInstructionCoder;
  readonly types: SplMemoTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplMemoAccountsCoder(idl);
    this.events = new SplMemoEventsCoder(idl);
    this.instruction = new SplMemoInstructionCoder(idl);
    this.types = new SplMemoTypesCoder(idl);
  }
}
