import BN from "bn.js";
import fetch from "cross-fetch";
import * as borsh from "@anchor-lang/borsh";
import { Connection, PublicKey } from "@solana/web3.js";

/**
 * Returns a verified build from the anchor registry. null if no such
 * verified build exists, e.g., if the program has been upgraded since the
 * last verified build.
 */
export async function verifiedBuild(
  connection: Connection,
  programId: PublicKey,
  limit: number = 5
): Promise<Build | null> {
  const url = `https://api.apr.dev/api/v0/program/${programId.toString()}/latest?limit=${limit}`;
  const [programData, latestBuildsResp] = await Promise.all([
    fetchData(connection, programId),
    fetch(url),
  ]);

  // Filter out all non successful builds.
  const latestBuilds = (await latestBuildsResp.json()).filter(
    (b: Build) => !b.aborted && b.state === "Built" && b.verified === "Verified"
  );
  if (latestBuilds.length === 0) {
    return null;
  }

  // Get the latest build.
  const build = latestBuilds[0];

  // Has the program been upgraded since the last build?
  if (programData.slot.toNumber() !== build.verified_slot) {
    return null;
  }

  // Success.
  return build;
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

export type Build = {
  aborted: boolean;
  address: string;
  created_at: string;
  updated_at: string;
  descriptor: string[];
  docker: string;
  id: number;
  name: string;
  sha256: string;
  upgrade_authority: string;
  verified: string;
  verified_slot: number;
  state: string;
};
