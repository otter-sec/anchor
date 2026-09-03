import { Idl, Coder } from "@anchor-lang/core";

import { SplBinaryOptionAccountsCoder } from "./accounts";
import { SplBinaryOptionEventsCoder } from "./events";
import { SplBinaryOptionInstructionCoder } from "./instructions";
import { SplBinaryOptionTypesCoder } from "./types";

/**
 * Coder for SplBinaryOption
 */
export class SplBinaryOptionCoder implements Coder {
  readonly accounts: SplBinaryOptionAccountsCoder;
  readonly events: SplBinaryOptionEventsCoder;
  readonly instruction: SplBinaryOptionInstructionCoder;
  readonly types: SplBinaryOptionTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplBinaryOptionAccountsCoder(idl);
    this.events = new SplBinaryOptionEventsCoder(idl);
    this.instruction = new SplBinaryOptionInstructionCoder(idl);
    this.types = new SplBinaryOptionTypesCoder(idl);
  }
}
