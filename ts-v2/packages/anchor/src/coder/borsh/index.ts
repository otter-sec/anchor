import { Idl } from "../../idl.js";
import { BorshInstructionCoder } from "./instruction.js";
import { BorshAccountsCoder } from "./accounts.js";
import { BorshEventCoder } from "./event.js";
import { BorshTypesCoder } from "./types.js";
import { Coder } from "../index.js";

export { BorshInstructionCoder } from "./instruction.js";
export { BorshAccountsCoder } from "./accounts.js";
export { BorshEventCoder } from "./event.js";

/**
 * BorshCoder is the default Coder for Anchor programs implementing the
 * borsh based serialization interface.
 */
export class BorshCoder<A extends string = string, T extends string = string>
  implements Coder
{
  /**
   * Instruction coder.
   */
  readonly instruction: BorshInstructionCoder;

  /**
   * Account coder.
   */
  readonly accounts: BorshAccountsCoder<A>;

  /**
   * Coder for events.
   */
  readonly events: BorshEventCoder;

  /**
   * Coder for user-defined types.
   */
  readonly types: BorshTypesCoder<T>;

  constructor(idl: Idl) {
    this.instruction = new BorshInstructionCoder(idl);
    this.accounts = new BorshAccountsCoder(idl);
    this.events = new BorshEventCoder(idl);
    this.types = new BorshTypesCoder(idl);
  }
}
