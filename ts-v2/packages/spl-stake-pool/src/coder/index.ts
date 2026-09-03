import { Idl, Coder } from "@anchor-lang/core";

import { SplStakePoolAccountsCoder } from "./accounts";
import { SplStakePoolEventsCoder } from "./events";
import { SplStakePoolInstructionCoder } from "./instructions";
import { SplStakePoolTypesCoder } from "./types";

/**
 * Coder for SplStakePool
 */
export class SplStakePoolCoder implements Coder {
  readonly accounts: SplStakePoolAccountsCoder;
  readonly events: SplStakePoolEventsCoder;
  readonly instruction: SplStakePoolInstructionCoder;
  readonly types: SplStakePoolTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplStakePoolAccountsCoder(idl);
    this.events = new SplStakePoolEventsCoder(idl);
    this.instruction = new SplStakePoolInstructionCoder(idl);
    this.types = new SplStakePoolTypesCoder(idl);
  }
}
