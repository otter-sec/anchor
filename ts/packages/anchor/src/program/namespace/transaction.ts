import {
  addSignersToTransactionMessage,
  appendTransactionMessageInstructions,
  createTransactionMessage,
  pipe,
  TransactionMessage,
  TransactionMessageWithSigners,
} from "@solana/kit";
import { Idl } from "../../idl.js";
import { splitArgsAndCtx } from "../context.js";
import { InstructionFn } from "./instruction.js";
import {
  AllInstructions,
  InstructionContextFn,
  MakeInstructionsNamespace,
} from "./types.js";

export default class TransactionFactory {
  public static build<IDL extends Idl, I extends AllInstructions<IDL>>(
    idlIx: I,
    ixFn: InstructionFn<IDL, I>
  ): TransactionFn<IDL, I> {
    const txFn: TransactionFn<IDL, I> = (
      ...args
    ): ProgramTransactionMessage => {
      const [, ctx] = splitArgsAndCtx(idlIx, [...args]);
      return pipe(
        createTransactionMessage({ version: 0 }),
        (message) =>
          appendTransactionMessageInstructions(
            [
              ...(ctx.preInstructions ?? []),
              ixFn(...args),
              ...(ctx.postInstructions ?? []),
            ],
            message
          ),
        (message) => addSignersToTransactionMessage(ctx.signers ?? [], message)
      );
    };

    return txFn;
  }
}

/**
 * A transaction message built by the transaction namespace: it carries the
 * instructions and any signers given in the context, but no fee payer or
 * lifetime. The provider adds both when sending; when signing manually,
 * set them before compiling.
 *
 * ```typescript
 * const message = await program.methods.increment().transactionMessage();
 * const transaction = await signTransactionMessageWithSigners(
 *   pipe(
 *     message,
 *     (m) => setTransactionMessageFeePayerSigner(wallet, m),
 *     (m) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m)
 *   )
 * );
 * ```
 */
export type ProgramTransactionMessage = TransactionMessage &
  TransactionMessageWithSigners;

/**
 * The namespace provides functions to build transaction messages for each
 * method of a program.
 *
 * ## Usage
 *
 * ```javascript
 * program.transaction.<method>(...args, ctx);
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
 * To create a transaction message for the `increment` method above,
 *
 * ```javascript
 * const message = await program.transaction.increment({
 *   accounts: {
 *     counter,
 *   },
 * });
 * ```
 */
export type TransactionNamespace<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = MakeInstructionsNamespace<IDL, I, ProgramTransactionMessage>;

/**
 * Tx is a function to create a transaction message for a given program
 * instruction.
 */
export type TransactionFn<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = InstructionContextFn<IDL, I, ProgramTransactionMessage>;
