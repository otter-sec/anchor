import {
  AccountMeta,
  ConfirmOptions,
  Signer,
  TransactionInstruction,
} from "@solana/web3.js";
import { Address } from "./common.js";
import {
  IdlInstructionAccountItem,
  IdlInstructionAccounts,
  IdlInstruction,
} from "../idl.js";

/**
 * Context provides all non-argument inputs for generating Anchor transactions.
 */
export type Context<A extends Accounts = Accounts> = {
  /**
   * Accounts used in the instruction context.
   */
  accounts?: A;

  /**
   * All accounts to pass into an instruction *after* the main `accounts`.
   * This can be used for optional or otherwise unknown accounts.
   */
  remainingAccounts?: AccountMeta[];

  /**
   * Accounts that must sign a given transaction.
   */
  signers?: Array<Signer>;

  /**
   * @deprecated use preInstructions instead.
   * Instructions to run *before* a given method. Often this is used, for
   * example to create accounts prior to executing a method.
   */
  instructions?: TransactionInstruction[];

  /**
   * Instructions to run *before* a given method. Often this is used, for
   * example to create accounts prior to executing a method.
   */
  preInstructions?: TransactionInstruction[];

  /**
   * Instructions to run *after* a given method. Often this is used, for
   * example to close accounts after executing a method.
   */
  postInstructions?: TransactionInstruction[];

  /**
   * Commitment parameters to use for a transaction.
   */
  options?: ConfirmOptions;
};

/**
 * A set of accounts mapping one-to-one to the program's accounts struct, i.e.,
 * the type deriving `#[derive(Accounts)]`.
 *
 * The name of each field should match the name for that account in the IDL.
 *
 * If multiple accounts are nested in the rust program, then they should be
 * nested here.
 */
export type Accounts<
  A extends IdlInstructionAccountItem = IdlInstructionAccountItem
> = {
  [N in A["name"]]: Account<A & { name: N }>;
};

type Account<A extends IdlInstructionAccountItem> =
  A extends IdlInstructionAccounts
    ? Accounts<A["accounts"][number]>
    : A extends { optional: true }
    ? Address | null
    : A extends { signer: true }
    ? Address | undefined
    : Address;

export function splitArgsAndCtx(
  idlIx: IdlInstruction,
  args: any[]
): [any[], Context] {
  let options = {};

  const inputLen = idlIx.args ? idlIx.args.length : 0;
  if (args.length > inputLen) {
    if (args.length !== inputLen + 1) {
      throw new Error(
        `provided too many arguments ${args} to instruction ${
          idlIx?.name
        } expecting: ${idlIx.args?.map((a) => a.name) ?? []}`
      );
    }
    options = args.pop();
  }

  return [args, options];
}
