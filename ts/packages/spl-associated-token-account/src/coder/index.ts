import { Idl, Coder } from "@anchor-lang/core";

import { SplAssociatedTokenAccountAccountsCoder } from "./accounts";
import { SplAssociatedTokenAccountEventsCoder } from "./events";
import { SplAssociatedTokenAccountInstructionCoder } from "./instructions";
import { SplAssociatedTokenAccountTypesCoder } from "./types";

/**
 * Coder for SplAssociatedTokenAccount
 */
export class SplAssociatedTokenAccountCoder implements Coder {
  readonly accounts: SplAssociatedTokenAccountAccountsCoder;
  readonly events: SplAssociatedTokenAccountEventsCoder;
  readonly instruction: SplAssociatedTokenAccountInstructionCoder;
  readonly types: SplAssociatedTokenAccountTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplAssociatedTokenAccountAccountsCoder(idl);
    this.events = new SplAssociatedTokenAccountEventsCoder(idl);
    this.instruction = new SplAssociatedTokenAccountInstructionCoder(idl);
    this.types = new SplAssociatedTokenAccountTypesCoder(idl);
  }
}
