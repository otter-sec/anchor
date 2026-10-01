import { Address, createAddressWithSeed } from "@solana/kit";
import { Address as AnchorAddress, toAddress } from "../program/common.js";

/**
 * Derives the address `sha256(base || seed || programId)`, as the system
 * program's `createAccountWithSeed` does.
 */
export async function createWithSeed(
  base: AnchorAddress,
  seed: string,
  programId: AnchorAddress
): Promise<Address> {
  return await createAddressWithSeed({
    baseAddress: toAddress(base),
    programAddress: toAddress(programId),
    seed,
  });
}
