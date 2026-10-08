import { Connection, PublicKey } from "@solana/web3.js";
import { Idl, Program } from "../src";

// Jest 27 does not expose Node's structuredClone in its test environment.
if (typeof globalThis.structuredClone !== "function") {
  globalThis.structuredClone = (value) => JSON.parse(JSON.stringify(value));
}

describe("Program.at", () => {
  const address = "11111111111111111111111111111111";
  const provider = { connection: new Connection("http://localhost:8899") };
  const idl: Idl = {
    address,
    metadata: { name: "test", version: "0.1.0", spec: "0.1.0" },
    instructions: [],
  };

  afterEach(() => jest.restoreAllMocks());

  it.each([address, new PublicKey(address)])(
    "accepts a matching IDL address for %s",
    async (requested) => {
      jest.spyOn(Program, "fetchIdl").mockResolvedValue(idl);

      const program = await Program.at(requested, provider);

      expect(program.programId.equals(new PublicKey(address))).toBe(true);
    }
  );

  it.each([address, new PublicKey(address)])(
    "rejects a mismatching IDL address for %s",
    async (requested) => {
      const otherAddress = "BPFLoaderUpgradeab1e11111111111111111111111";
      jest
        .spyOn(Program, "fetchIdl")
        .mockResolvedValue({ ...idl, address: otherAddress });

      await expect(Program.at(requested, provider)).rejects.toThrow(
        `IDL address ${otherAddress} does not match requested program ${address}`
      );
    }
  );
});
