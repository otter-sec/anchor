import { Idl, Coder } from "@anchor-lang/core";

import { SplTokenLendingAccountsCoder } from "./accounts";
import { SplTokenLendingEventsCoder } from "./events";
import { SplTokenLendingInstructionCoder } from "./instructions";
import { SplTokenLendingTypesCoder } from "./types";

/**
 * Coder for SplTokenLending
 */
export class SplTokenLendingCoder implements Coder {
  readonly accounts: SplTokenLendingAccountsCoder;
  readonly events: SplTokenLendingEventsCoder;
  readonly instruction: SplTokenLendingInstructionCoder;
  readonly types: SplTokenLendingTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplTokenLendingAccountsCoder(idl);
    this.events = new SplTokenLendingEventsCoder(idl);
    this.instruction = new SplTokenLendingInstructionCoder(idl);
    this.types = new SplTokenLendingTypesCoder(idl);
  }
}
