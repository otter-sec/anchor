import { Idl, Coder } from "@anchor-lang/core";

import { SplFeatureProposalAccountsCoder } from "./accounts";
import { SplFeatureProposalEventsCoder } from "./events";
import { SplFeatureProposalInstructionCoder } from "./instructions";
import { SplFeatureProposalTypesCoder } from "./types";

/**
 * Coder for SplFeatureProposal
 */
export class SplFeatureProposalCoder implements Coder {
  readonly accounts: SplFeatureProposalAccountsCoder;
  readonly events: SplFeatureProposalEventsCoder;
  readonly instruction: SplFeatureProposalInstructionCoder;
  readonly types: SplFeatureProposalTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplFeatureProposalAccountsCoder(idl);
    this.events = new SplFeatureProposalEventsCoder(idl);
    this.instruction = new SplFeatureProposalInstructionCoder(idl);
    this.types = new SplFeatureProposalTypesCoder(idl);
  }
}
