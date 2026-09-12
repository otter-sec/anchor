import { Idl, Coder } from "@anchor-lang/core";

import { SplTokenAccountsCoder } from "./accounts";
import { SplTokenEventsCoder } from "./events";
import { SplTokenInstructionCoder } from "./instructions";
import { SplTokenTypesCoder } from "./types";

/**
 * Coder for SplToken
 */
export class SplTokenCoder implements Coder {
  readonly accounts: SplTokenAccountsCoder;
  readonly events: SplTokenEventsCoder;
  readonly instruction: SplTokenInstructionCoder;
  readonly types: SplTokenTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplTokenAccountsCoder(idl);
    this.events = new SplTokenEventsCoder(idl);
    this.instruction = new SplTokenInstructionCoder(idl);
    this.types = new SplTokenTypesCoder(idl);
  }
}
