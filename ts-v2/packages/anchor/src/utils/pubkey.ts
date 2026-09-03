import { PublicKey } from "@solana/web3.js";
import { sha256 } from "@noble/hashes/sha256";

// Sync version of web3.PublicKey.createWithSeed.
export function createWithSeedSync(
  fromPublicKey: PublicKey,
  seed: string,
  programId: PublicKey
): PublicKey {
  const buffer = Buffer.concat([
    fromPublicKey.toBuffer(),
    Buffer.from(seed),
    programId.toBuffer(),
  ]);
  return new PublicKey(sha256(buffer));
}
