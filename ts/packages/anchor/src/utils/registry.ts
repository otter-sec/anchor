import fetch from "cross-fetch";
import { Address, getStructCodec, getU32Codec, getU64Codec } from "@solana/kit";
import { Connection, PublicKey } from "@solana/web3.js";
import {
  getAnchorOptionCodec,
  getPublicKeyCodec,
  getRustEnumCodec,
} from "../coder/borsh/codecs.js";

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
  if (Number(programData.slot) !== build.verified_slot) {
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
    new PublicKey(program.programdataAddress)
  );
  if (programdataAccountInfo === null) {
    throw new Error("program data account not found");
  }
  const { programData } = decodeUpgradeableLoaderState(
    programdataAccountInfo.data
  );
  return programData;
}

// The BPF upgradeable loader state enum uses a u32 discriminant, unlike
// borsh's default u8.
const UPGRADEABLE_LOADER_STATE_CODEC = getRustEnumCodec(
  [
    ["uninitialized", getStructCodec([])],
    [
      "buffer",
      getStructCodec([
        ["authorityAddress", getAnchorOptionCodec(getPublicKeyCodec())],
      ]),
    ],
    ["program", getStructCodec([["programdataAddress", getPublicKeyCodec()]])],
    [
      "programData",
      getStructCodec([
        ["slot", getU64Codec()],
        ["upgradeAuthorityAddress", getAnchorOptionCodec(getPublicKeyCodec())],
      ]),
    ],
  ],
  getU32Codec()
);

export function decodeUpgradeableLoaderState(data: Buffer): any {
  return UPGRADEABLE_LOADER_STATE_CODEC.decode(data);
}

export type ProgramData = {
  slot: bigint;
  upgradeAuthorityAddress: Address | null;
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
