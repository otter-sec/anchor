import { Idl } from "../../idl.js";
import { Coder } from "../index.js";
import { SystemInstructionCoder } from "./instruction.js";
import { SystemAccountsCoder } from "./accounts.js";
import { SystemEventsCoder } from "./events.js";
import { SystemTypesCoder } from "./types.js";

/**
 * Coder for the System program.
 */
export class SystemCoder implements Coder {
  readonly instruction: SystemInstructionCoder;
  readonly accounts: SystemAccountsCoder;
  readonly events: SystemEventsCoder;
  readonly types: SystemTypesCoder;

  constructor(idl: Idl) {
    this.instruction = new SystemInstructionCoder(idl);
    this.accounts = new SystemAccountsCoder(idl);
    this.events = new SystemEventsCoder(idl);
    this.types = new SystemTypesCoder(idl);
  }
}
