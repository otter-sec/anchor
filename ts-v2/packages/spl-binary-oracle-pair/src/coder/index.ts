import { Idl, Coder } from "@anchor-lang/core";

import { SplBinaryOraclePairAccountsCoder } from "./accounts";
import { SplBinaryOraclePairEventsCoder } from "./events";
import { SplBinaryOraclePairInstructionCoder } from "./instructions";
import { SplBinaryOraclePairTypesCoder } from "./types";

/**
 * Coder for SplBinaryOraclePair
 */
export class SplBinaryOraclePairCoder implements Coder {
  readonly accounts: SplBinaryOraclePairAccountsCoder;
  readonly events: SplBinaryOraclePairEventsCoder;
  readonly instruction: SplBinaryOraclePairInstructionCoder;
  readonly types: SplBinaryOraclePairTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplBinaryOraclePairAccountsCoder(idl);
    this.events = new SplBinaryOraclePairEventsCoder(idl);
    this.instruction = new SplBinaryOraclePairInstructionCoder(idl);
    this.types = new SplBinaryOraclePairTypesCoder(idl);
  }
}
