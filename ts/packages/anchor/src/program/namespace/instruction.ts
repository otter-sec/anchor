import {
  AccountMeta,
  AccountRole,
  Address,
  Instruction,
  ReadonlyUint8Array,
} from "@solana/kit";
import {
  Idl,
  IdlInstructionAccountItem,
  IdlInstructionAccounts,
  IdlInstruction,
  isCompositeAccounts,
} from "../../idl.js";
import { IdlError } from "../../error.js";
import {
  AddressInput,
  toAddress,
  toInstruction,
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
    programAddress: Address
  ): InstructionFn<IDL, I> {
    if (idlIx.name === "_inner") {
      throw new IdlError("the _inner name is reserved");
    }

    const ix = (...args: InstructionContextFnArgs<IDL, I>): Instruction => {
      const [ixArgs, ctx] = splitArgsAndCtx(idlIx, [...args]);
      validateAccounts(idlIx.accounts, ctx.accounts);
      validateInstruction(idlIx, ...args);

      const accounts = ix.accounts(ctx.accounts);

      if (ctx.remainingAccounts !== undefined) {
        accounts.push(...ctx.remainingAccounts);
      }

      if (features.isSet("debug-logs")) {
        console.log("Outgoing account metas:", accounts);
      }

      return {
        programAddress,
        accounts,
        data: encodeFn(idlIx.name, toInstruction(idlIx, ...ixArgs)),
      };
    };

    // Utility fn for ordering the accounts for this instruction.
    ix["accounts"] = (accs: Accounts<I["accounts"][number]> | undefined) => {
      return InstructionNamespaceFactory.accountsArray(
        accs,
        idlIx.accounts,
        programAddress,
        idlIx.name
      );
    };

    return ix;
  }

  public static accountsArray(
    ctx: Accounts | undefined,
    accounts: readonly IdlInstructionAccountItem[],
    programAddress: Address,
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
            programAddress,
            ixName
          ).flat();
        }

        let address: Address;
        try {
          address = toAddress(ctx[acc.name] as AddressInput);
        } catch (err) {
          throw new Error(
            `Wrong input type for account "${
              acc.name
            }" in the instruction accounts object${
              ixName !== undefined ? ' for instruction "' + ixName + '"' : ""
            }. Expected an address.`
          );
        }

        // Optional accounts left unset are passed as the program itself,
        // which must then be neither writable nor a signer.
        const isOptional = acc.optional && address === programAddress;
        const isWritable = Boolean(acc.writable && !isOptional);
        const isSigner = Boolean(acc.signer && !isOptional);
        return { address, role: toAccountRole(isWritable, isSigner) };
      })
      .flat();
  }
}

function toAccountRole(isWritable: boolean, isSigner: boolean): AccountRole {
  if (isSigner) {
    return isWritable
      ? AccountRole.WRITABLE_SIGNER
      : AccountRole.READONLY_SIGNER;
  }
  return isWritable ? AccountRole.WRITABLE : AccountRole.READONLY;
}

/**
 * The namespace provides functions to build Kit `Instruction` objects for
 * each method of a program.
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
  Instruction,
  {
    [M in keyof AllInstructionsMap<IDL>]: {
      accounts: (
        ctx: Accounts<AllInstructionsMap<IDL>[M]["accounts"][number]>
      ) => unknown;
    };
  }
>;

/**
 * Function to create a Kit `Instruction` generated from an IDL.
 * Additionally it provides an `accounts` utility method, returning a list
 * of ordered accounts for the instruction.
 */
export type InstructionFn<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = InstructionContextFn<IDL, I, Instruction> &
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
) => ReadonlyUint8Array;

// Throws error if any argument required for the `ix` is not given.
function validateInstruction(ix: IdlInstruction, ...args: any[]) {
  // todo
}
