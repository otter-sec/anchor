import { Signature } from "@solana/kit";
import Provider from "../../provider.js";
import { Idl } from "../../idl.js";
import { splitArgsAndCtx } from "../context.js";
import { TransactionFn } from "./transaction.js";
import { translateError } from "../../error.js";
import {
  AllInstructions,
  InstructionContextFn,
  MakeInstructionsNamespace,
} from "./types.js";

export default class RpcFactory {
  public static build<IDL extends Idl, I extends AllInstructions<IDL>>(
    idlIx: I,
    txFn: TransactionFn<IDL, I>,
    idlErrors: Map<number, string>,
    provider: Provider
  ): RpcFn {
    const rpc: RpcFn<IDL, I> = async (...args) => {
      const message = txFn(...args);
      const [, ctx] = splitArgsAndCtx(idlIx, [...args]);
      if (provider.sendAndConfirm === undefined) {
        throw new Error(
          "This function requires 'Provider.sendAndConfirm' to be implemented."
        );
      }
      try {
        // The context signers are already attached to the message.
        return await provider.sendAndConfirm(message, [], ctx.options);
      } catch (err) {
        throw translateError(err, idlErrors);
      }
    };

    return rpc;
  }
}

/**
 * The namespace provides async methods to send signed transactions for each
 * *non*-state method on Anchor program.
 *
 * Keys are method names, values are RPC functions returning the signature of
 * the sent transaction.
 *
 * ## Usage
 *
 * ```javascript
 * rpc.<method>(...args, ctx);
 * ```
 *
 * ## Parameters
 *
 * 1. `args` - The positional arguments for the program. The type and number
 *    of these arguments depend on the program being used.
 * 2. `ctx`  - [[Context]] non-argument parameters to pass to the method.
 *    Always the last parameter in the method call.
 * ```
 *
 * ## Example
 *
 * To send a transaction invoking the `increment` method above,
 *
 * ```javascript
 * const txSignature = await program.rpc.increment({
 *   accounts: {
 *     counter,
 *     authority,
 *   },
 * });
 * ```
 */
export type RpcNamespace<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = MakeInstructionsNamespace<IDL, I, Promise<Signature>>;

/**
 * RpcFn is a single RPC method generated from an IDL, sending a transaction
 * paid for and signed by the configured provider.
 */
export type RpcFn<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = InstructionContextFn<IDL, I, Promise<Signature>>;
