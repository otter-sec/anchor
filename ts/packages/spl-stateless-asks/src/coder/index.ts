import { Idl, Coder } from "@anchor-lang/core";

import { SplStatelessAsksAccountsCoder } from "./accounts";
import { SplStatelessAsksEventsCoder } from "./events";
import { SplStatelessAsksInstructionCoder } from "./instructions";
import { SplStatelessAsksTypesCoder } from "./types";

/**
 * Coder for SplStatelessAsks
 */
export class SplStatelessAsksCoder implements Coder {
  readonly accounts: SplStatelessAsksAccountsCoder;
  readonly events: SplStatelessAsksEventsCoder;
  readonly instruction: SplStatelessAsksInstructionCoder;
  readonly types: SplStatelessAsksTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplStatelessAsksAccountsCoder(idl);
    this.events = new SplStatelessAsksEventsCoder(idl);
    this.instruction = new SplStatelessAsksInstructionCoder(idl);
    this.types = new SplStatelessAsksTypesCoder(idl);
  }
}
