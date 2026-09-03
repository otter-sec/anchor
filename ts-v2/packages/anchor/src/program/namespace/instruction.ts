import {
  AccountMeta,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  Idl,
  IdlInstructionAccountItem,
  IdlInstructionAccounts,
  IdlInstruction,
  isCompositeAccounts,
} from "../../idl.js";
import { IdlError } from "../../error.js";
import {
  Address,
  toInstruction,
  translateAddress,
  validateAccounts,
} from "../common.js";
import { Accounts, splitArgsAndCtx } from "../context.js";
import * as features from "../../utils/features.js";
import {
  AllInstructions,
  AllInstructionsMap,
  InstructionContextFn,
  InstructionContextFnArgs,
  MakeInstructionsNamespace,
} from "./types.js";

export default class InstructionNamespaceFactory {
  public static build<IDL extends Idl, I extends AllInstructions<IDL>>(
    idlIx: I,
    encodeFn: InstructionEncodeFn<I>,
    programId: PublicKey
  ): InstructionFn<IDL, I> {
    if (idlIx.name === "_inner") {
      throw new IdlError("the _inner name is reserved");
    }

    const ix = (
      ...args: InstructionContextFnArgs<IDL, I>
    ): TransactionInstruction => {
      const [ixArgs, ctx] = splitArgsAndCtx(idlIx, [...args]);
      validateAccounts(idlIx.accounts, ctx.accounts);
      validateInstruction(idlIx, ...args);

      const keys = ix.accounts(ctx.accounts);

      if (ctx.remainingAccounts !== undefined) {
        keys.push(...ctx.remainingAccounts);
      }

      if (features.isSet("debug-logs")) {
        console.log("Outgoing account metas:", keys);
      }

      return new TransactionInstruction({
        keys,
        programId,
        data: encodeFn(idlIx.name, toInstruction(idlIx, ...ixArgs)),
      });
    };

    // Utility fn for ordering the accounts for this instruction.
    ix["accounts"] = (accs: Accounts<I["accounts"][number]> | undefined) => {
      return InstructionNamespaceFactory.accountsArray(
        accs,
        idlIx.accounts,
        programId,
        idlIx.name
      );
    };

    return ix;
  }

  public static accountsArray(
    ctx: Accounts | undefined,
    accounts: readonly IdlInstructionAccountItem[],
    programId: PublicKey,
    ixName?: string
  ): AccountMeta[] {
    if (!ctx) {
      return [];
    }

    return accounts
      .map((acc) => {
        if (isCompositeAccounts(acc)) {
          const rpcAccs = ctx[acc.name] as Accounts;
          return InstructionNamespaceFactory.accountsArray(
            rpcAccs,
            (acc as IdlInstructionAccounts).accounts,
            programId,
            ixName
          ).flat();
        }

        let pubkey: PublicKey;
        try {
          pubkey = translateAddress(ctx[acc.name] as Address);
        } catch (err) {
          throw new Error(
            `Wrong input type for account "${
              acc.name
            }" in the instruction accounts object${
              ixName !== undefined ? ' for instruction "' + ixName + '"' : ""
            }. Expected PublicKey or string.`
          );
        }

        const isOptional = acc.optional && pubkey.equals(programId);
        const isWritable = Boolean(acc.writable && !isOptional);
        const isSigner = Boolean(acc.signer && !isOptional);
        return {
          pubkey,
          isWritable,
          isSigner,
        };
      })
      .flat();
  }
}

/**
 * The namespace provides functions to build [[TransactionInstruction]]
 * objects for each method of a program.
 *
 * ## Usage
 *
 * ```javascript
 * instruction.<method>(...args, ctx);
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
 * To create an instruction for the `increment` method above,
 *
 * ```javascript
 * const tx = await program.instruction.increment({
 *   accounts: {
 *     counter,
 *   },
 * });
 * ```
 */
export type InstructionNamespace<
  IDL extends Idl = Idl,
  I extends IdlInstruction = AllInstructions<IDL>
> = MakeInstructionsNamespace<
  IDL,
  I,
  TransactionInstruction,
  {
    [M in keyof AllInstructionsMap<IDL>]: {
      accounts: (
        ctx: Accounts<AllInstructionsMap<IDL>[M]["accounts"][number]>
      ) => unknown;
    };
  }
>;

/**
 * Function to create a `TransactionInstruction` generated from an IDL.
 * Additionally it provides an `accounts` utility method, returning a list
 * of ordered accounts for the instruction.
 */
export type InstructionFn<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = InstructionContextFn<IDL, I, TransactionInstruction> &
  IxProps<Accounts<I["accounts"][number]>>;

type IxProps<A extends Accounts> = {
  /**
   * Returns an ordered list of accounts associated with the instruction.
   */
  accounts: (ctx: A) => AccountMeta[];
};

export type InstructionEncodeFn<I extends IdlInstruction = IdlInstruction> = (
  ixName: I["name"],
  ix: any
) => Buffer;

// Throws error if any argument required for the `ix` is not given.
function validateInstruction(ix: IdlInstruction, ...args: any[]) {
  // todo
}
