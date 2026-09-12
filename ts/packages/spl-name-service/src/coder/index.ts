import { Idl, Coder } from "@anchor-lang/core";

import { SplNameServiceAccountsCoder } from "./accounts";
import { SplNameServiceEventsCoder } from "./events";
import { SplNameServiceInstructionCoder } from "./instructions";
import { SplNameServiceTypesCoder } from "./types";

/**
 * Coder for SplNameService
 */
export class SplNameServiceCoder implements Coder {
  readonly accounts: SplNameServiceAccountsCoder;
  readonly events: SplNameServiceEventsCoder;
  readonly instruction: SplNameServiceInstructionCoder;
  readonly types: SplNameServiceTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new SplNameServiceAccountsCoder(idl);
    this.events = new SplNameServiceEventsCoder(idl);
    this.instruction = new SplNameServiceInstructionCoder(idl);
    this.types = new SplNameServiceTypesCoder(idl);
  }
}
