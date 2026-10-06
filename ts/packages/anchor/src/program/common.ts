import { address, Address } from "@solana/kit";
import {
  Idl,
  IdlInstruction,
  IdlInstructionAccountItem,
  isCompositeAccounts,
} from "../idl.js";
import { Accounts } from "./context.js";

export function parseIdlErrors(idl: Idl): Map<number, string> {
  const errors = new Map();
  if (idl.errors) {
    idl.errors.forEach((e) => {
      let msg = e.msg ?? e.name;
      errors.set(e.code, msg);
    });
  }
  return errors;
}

export function toInstruction(idlIx: IdlInstruction, ...args: any[]) {
  if (idlIx.args.length != args.length) {
    throw new Error("Invalid argument length");
  }
  const ix: { [key: string]: any } = {};
  let idx = 0;
  idlIx.args.forEach((ixArg) => {
    ix[ixArg.name] = args[idx];
    idx += 1;
  });

  return ix;
}

// Throws error if any account required for the `ix` is not given.
export function validateAccounts(
  ixAccounts: IdlInstructionAccountItem[],
  accounts: Accounts = {}
) {
  ixAccounts.forEach((acc) => {
    if (isCompositeAccounts(acc)) {
      validateAccounts(acc.accounts, accounts[acc.name] as Accounts);
    } else {
      if (!accounts[acc.name]) {
        throw new Error(`Account \`${acc.name}\` not provided.`);
      }
    }
  });
}

/**
 * An address as accepted by the client: a Kit `Address`, or an object
 * exposing one through `toBase58()` (e.g. a web3.js public key), so that
 * values from libraries not yet on Kit can be passed as they are. Outputs
 * are always Kit addresses.
 */
export type AddressInput = Address | { toBase58(): string };

/**
 * Translates an address input to a Kit `Address`, validating it on the way.
 */
export function toAddress(input: AddressInput): Address {
  return address(typeof input === "string" ? input : input.toBase58());
}

/**
 * Whether the value is an object exposing its address through a `toBase58()`
 * method, e.g. a web3.js public key. Checks for the method rather than the
 * property name, so an accounts object with an account named `toBase58` is
 * not mistaken for one.
 */
export function hasToBase58(value: unknown): value is { toBase58(): string } {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { toBase58?: unknown }).toBase58 === "function"
  );
}
