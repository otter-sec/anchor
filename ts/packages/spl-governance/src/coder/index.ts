import { Idl, Coder } from "@anchor-lang/core";

import { SplGovernanceAccountsCoder } from "./accounts";
import { SplGovernanceEventsCoder } from "./events";
import { SplGovernanceInstructionCoder } from "./instructions";
import { SplGovernanceTypesCoder } from "./types";

/**
 * Coder for SplGovernance
 */
export class SplGovernanceCoder implements Coder {
  readonly accounts: SplGovernanceAccountsCoder;
  readonly events: SplGovernanceEventsCoder;
  readonly instruction: SplGovernanceInstructionCoder;
  readonly types: SplGovernanceTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplGovernanceAccountsCoder(idl);
    this.events = new SplGovernanceEventsCoder(idl);
    this.instruction = new SplGovernanceInstructionCoder(idl);
    this.types = new SplGovernanceTypesCoder(idl);
  }
}
