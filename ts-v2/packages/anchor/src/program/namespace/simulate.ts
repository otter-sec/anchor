import { PublicKey } from "@solana/web3.js";
import Provider from "../../provider.js";
import { SuccessfulTxSimulationResponse } from "src/utils/rpc.js";
import { splitArgsAndCtx } from "../context.js";
import { TransactionFn } from "./transaction.js";
import { EventParser, Event } from "../event.js";
import { Coder } from "../../coder/index.js";
import { Idl } from "../../idl.js";
import { translateError } from "../../error.js";
import {
  AllInstructions,
  InstructionContextFn,
  MakeInstructionsNamespace,
} from "./types";

export default class SimulateFactory {
  public static build<IDL extends Idl, I extends AllInstructions<IDL>>(
    idlIx: AllInstructions<IDL>,
    txFn: TransactionFn<IDL>,
    idlErrors: Map<number, string>,
    provider: Provider,
    coder: Coder,
    programId: PublicKey,
    idl: IDL
  ): SimulateFn<IDL, I> {
    const simulate: SimulateFn<IDL> = async (...args) => {
      const tx = txFn(...args);
      const [, ctx] = splitArgsAndCtx(idlIx, [...args]);
      let resp: SuccessfulTxSimulationResponse | undefined = undefined;
      if (provider.simulate === undefined) {
        throw new Error(
          "This function requires 'Provider.simulate' to be implemented."
        );
      }
      try {
        resp = await provider!.simulate(
          tx,
          ctx.signers,
          ctx.options?.commitment
        );
      } catch (err) {
        throw translateError(err, idlErrors);
      }
      if (resp === undefined) {
        throw new Error("Unable to simulate transaction");
      }
      const logs = resp.logs;
      if (!logs) {
        throw new Error("Simulated logs not found");
      }

      const events: Event[] = [];
      if (idl.events) {
        let parser = new EventParser(programId, coder);
        for (const event of parser.parseLogs(logs)) {
          events.push(event);
        }
      }
      return { events, raw: logs };
    };

    return simulate;
  }
}

/**
 * The namespace provides functions to simulate transactions for each method
 * of a program, returning a list of deserialized events *and* raw program
 * logs.
 *
 * One can use this to read data calculated from a program on chain, by
 * emitting an event in the program and reading the emitted event client side
 * via the `simulate` namespace.
 *
 * ## Usage
 *
 * ```javascript
 * program.simulate.<method>(...args, ctx);
 * ```
 *
 * ## Parameters
 *
 * 1. `args` - The positional arguments for the program. The type and number
 *    of these arguments depend on the program being used.
 * 2. `ctx`  - [[Context]] non-argument parameters to pass to the method.
 *    Always the last parameter in the method call.
 *
 * ## Example
 *
 * To simulate the `increment` method above,
 *
 * ```javascript
 * const events = await program.simulate.increment({
 *   accounts: {
 *     counter,
 *   },
 * });
 * ```
 */
export type SimulateNamespace<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = MakeInstructionsNamespace<IDL, I, Promise<SimulateResponse>>;

/**
 * SimulateFn is a single method generated from an IDL. It simulates a method
 * against a cluster configured by the provider, returning a list of all the
 * events and raw logs that were emitted during the execution of the
 * method.
 */
export type SimulateFn<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = InstructionContextFn<IDL, I, Promise<SimulateResponse>>;

// TODO: Infer event types
export type SimulateResponse = {
  events: readonly Event[];
  raw: readonly string[];
};
