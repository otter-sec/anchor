import { getBase64Decoder, getUtf8Encoder } from "@solana/kit";
import {
  Compression,
  DataSource,
  Encoding,
  findCanonicalPda,
  Format,
  getMetadataEncoder,
  MetadataArgs,
} from "@solana-program/program-metadata";
import { deflate, gzip } from "pako";
import { Idl, Program } from "../src";
import { mockProvider, randomAddress } from "./helpers/mock-provider";

const idl: Idl = {
  address: "Test111111111111111111111111111111111111111",
  metadata: { name: "counter", version: "0.1.0", spec: "0.1.0" },
  instructions: [],
};
const json = getUtf8Encoder().encode(JSON.stringify(idl));

/** RPC account info for a program metadata account holding `data`. */
function metadataAccount(
  program: string,
  data: Uint8Array,
  overrides: Partial<MetadataArgs> = {}
) {
  const bytes = getMetadataEncoder().encode({
    program: program as any,
    authority: null,
    mutable: true,
    canonical: true,
    seed: "idl",
    encoding: Encoding.Utf8,
    compression: Compression.None,
    format: Format.Json,
    dataSource: DataSource.Direct,
    dataLength: data.length,
    data,
    ...overrides,
  });
  return {
    context: { slot: 1 },
    value: {
      data: [getBase64Decoder().decode(bytes), "base64"],
      executable: false,
      lamports: 1,
      owner: "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S",
      rentEpoch: 0,
      space: bytes.length,
    },
  };
}

describe("Program.fetchIdl", () => {
  it("reads the canonical idl metadata of the program at the provider commitment", async () => {
    const program = randomAddress();
    const { provider, requests } = mockProvider({
      getAccountInfo: () => metadataAccount(program, json as Uint8Array),
    });

    const fetched = await Program.fetchIdl(program, provider);

    expect(fetched).toEqual(idl);
    const [expected] = await findCanonicalPda({ program, seed: "idl" });
    const request = requests.find((r) => r.method === "getAccountInfo")!;
    expect(request.params[0]).toBe(expected);
    expect(request.params[1]).toMatchObject({
      encoding: "base64",
      commitment: "confirmed",
    });
  });

  it.each([
    ["none", Compression.None, (data: Uint8Array) => data],
    ["gzip", Compression.Gzip, gzip],
    ["zlib", Compression.Zlib, deflate],
  ])("reads IDLs stored with %s compression", async (_, compression, pack) => {
    const program = randomAddress();
    const { provider } = mockProvider({
      getAccountInfo: () =>
        metadataAccount(program, pack(json as Uint8Array), { compression }),
    });

    expect(await Program.fetchIdl(program, provider)).toEqual(idl);
  });

  it("returns null when the program has no idl metadata", async () => {
    const { provider } = mockProvider({
      getAccountInfo: () => ({ context: { slot: 1 }, value: null }),
    });

    expect(await Program.fetchIdl(randomAddress(), provider)).toBeNull();
  });

  it("rejects other formats and data sources", async () => {
    const program = randomAddress();
    const { provider } = mockProvider({
      getAccountInfo: () =>
        metadataAccount(program, json as Uint8Array, { format: Format.Yaml }),
    });
    await expect(Program.fetchIdl(program, provider)).rejects.toThrow(
      "only JSON IDLs"
    );

    const { provider: other } = mockProvider({
      getAccountInfo: () =>
        metadataAccount(program, json as Uint8Array, {
          dataSource: DataSource.Url,
        }),
    });
    await expect(Program.fetchIdl(program, other)).rejects.toThrow(
      "only directly embedded data"
    );
  });
});
