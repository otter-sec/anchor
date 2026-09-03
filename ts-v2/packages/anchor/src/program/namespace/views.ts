import { PublicKey } from "@solana/web3.js";
import { Idl, IdlInstructionAccountItem, isCompositeAccounts } from "../../idl.js";
import { SimulateFn } from "./simulate.js";
import {
  AllInstructions,
  InstructionContextFn,
  MakeInstructionsNamespace,
} from "./types";
import { IdlCoder } from "../../coder/borsh/idl";
import { decode } from "../../utils/bytes/base64";

// Recursively walk composite account groups so a nested `#[account(mut)]`
// still disqualifies an instruction from being surfaced as a view.
function hasWritableAccount(accounts: IdlInstructionAccountItem[]): boolean {
  return accounts.some((a) =>
    isCompositeAccounts(a) ? hasWritableAccount(a.accounts) : a.writable === true
  );
}

export default class ViewFactory {
  public static build<IDL extends Idl, I extends AllInstructions<IDL>>(
    programId: PublicKey,
    idlIx: AllInstructions<IDL>,
    simulateFn: SimulateFn<IDL>,
    idl: IDL
  ): ViewFn<IDL, I> | undefined {
    const isWritable = hasWritableAccount(idlIx.accounts);
    const hasReturn = !!idlIx.returns;
    if (isWritable || !hasReturn) return;

    const view: ViewFn<IDL> = async (...args) => {
      let simulationResult = await simulateFn(...args);
      const returnPrefix = `Program return: ${programId} `;
      let returnLog = simulationResult.raw.find((l) =>
        l.startsWith(returnPrefix)
      );
      if (!returnLog) {
        throw new Error("View expected return log");
      }

      let returnData = decode(returnLog.slice(returnPrefix.length));
      let returnType = idlIx.returns;
      if (!returnType) {
        throw new Error("View expected return type");
      }

      const coder = IdlCoder.fieldLayout({ type: returnType }, idl.types);
      return coder.decode(returnData);
    };
    return view;
  }
}

export type ViewNamespace<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = MakeInstructionsNamespace<IDL, I, Promise<any>>;

/**
 * ViewFn is a single method generated from an IDL. It simulates a method
 * against a cluster configured by the provider, and then parses the events
 * and extracts return data from the raw logs emitted during the simulation.
 */
export type ViewFn<
  IDL extends Idl = Idl,
  I extends AllInstructions<IDL> = AllInstructions<IDL>
> = InstructionContextFn<IDL, I, Promise<any>>;
