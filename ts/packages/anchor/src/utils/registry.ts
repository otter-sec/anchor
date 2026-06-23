import BN from "bn.js";
import * as borsh from "@anchor-lang/borsh";
import { Connection, PublicKey } from "@solana/web3.js";

const OSEC_REGISTRY_URL = "https://verify.osec.io";

export type VerifiedBuild = {
  is_verified: boolean;
  message: string;
  on_chain_hash: string;
  executable_hash: string;
  repo_url: string;
  commit: string;
  last_verified_at: string | null;
  is_frozen: boolean;
  is_closed: boolean;
};

/** Returns verified build info from the OtterSec registry, or null if unverified or the request fails. */
export async function verifiedBuild(
  programId: PublicKey
): Promise<VerifiedBuild | null> {
  try {
    const resp = await fetch(
      `${OSEC_REGISTRY_URL}/status/${programId.toString()}`
    );
    if (!resp.ok) return null;
    const build = (await resp.json()) as VerifiedBuild;
    return build.is_verified ? build : null;
  } catch {
    return null;
  }
}

/**
 * Returns the program data account for this program, containing the
 * metadata for this program, e.g., the upgrade authority.
 */
export async function fetchData(
  connection: Connection,
  programId: PublicKey
): Promise<ProgramData> {
  const accountInfo = await connection.getAccountInfo(programId);
  if (accountInfo === null) {
    throw new Error("program account not found");
  }
  const { program } = decodeUpgradeableLoaderState(accountInfo.data);
  const programdataAccountInfo = await connection.getAccountInfo(
    program.programdataAddress
  );
  if (programdataAccountInfo === null) {
    throw new Error("program data account not found");
  }
  const { programData } = decodeUpgradeableLoaderState(
    programdataAccountInfo.data
  );
  return programData;
}

const UPGRADEABLE_LOADER_STATE_LAYOUT = borsh.rustEnum(
  [
    borsh.struct([], "uninitialized"),
    borsh.struct(
      [borsh.option(borsh.publicKey(), "authorityAddress")],
      "buffer"
    ),
    borsh.struct([borsh.publicKey("programdataAddress")], "program"),
    borsh.struct(
      [
        borsh.u64("slot"),
        borsh.option(borsh.publicKey(), "upgradeAuthorityAddress"),
      ],
      "programData"
    ),
  ],
  undefined,
  borsh.u32()
);

export function decodeUpgradeableLoaderState(data: Buffer): any {
  return UPGRADEABLE_LOADER_STATE_LAYOUT.decode(data);
}

export type ProgramData = {
  slot: BN;
  upgradeAuthorityAddress: PublicKey | null;
};
