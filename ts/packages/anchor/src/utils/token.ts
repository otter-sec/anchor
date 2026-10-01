import { Address } from "@solana/kit";
import {
  ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
  findAssociatedTokenPda,
  TOKEN_PROGRAM_ADDRESS,
} from "@solana-program/token";
import { Address as AnchorAddress, toAddress } from "../program/common.js";

export const TOKEN_PROGRAM_ID: Address = TOKEN_PROGRAM_ADDRESS;
export const ASSOCIATED_PROGRAM_ID: Address = ASSOCIATED_TOKEN_PROGRAM_ADDRESS;

/**
 * Derives the associated token account address of the given mint and owner,
 * under the SPL token program unless another token program (e.g. Token-2022)
 * is given.
 */
export async function associatedAddress({
  mint,
  owner,
  tokenProgram = TOKEN_PROGRAM_ADDRESS,
}: {
  mint: AnchorAddress;
  owner: AnchorAddress;
  tokenProgram?: AnchorAddress;
}): Promise<Address> {
  const [address] = await findAssociatedTokenPda({
    mint: toAddress(mint),
    owner: toAddress(owner),
    tokenProgram: toAddress(tokenProgram),
  });
  return address;
}
