import { PublicKey } from "@solana/web3.js";
import { Inflate } from "pako";
import { deflateSync, gzipSync } from "zlib";

import {
  DEFAULT_MAX_IDL_DATA_SIZE,
  FORMAT_JSON,
  Idl,
  MetadataCompression,
  MetadataEncoding,
  Program,
  decodeIdlAccount,
  idlAddress,
} from "../src";

// Jest 27 does not expose Node's structuredClone in its test environment.
if (typeof globalThis.structuredClone !== "function") {
  globalThis.structuredClone = (value) => JSON.parse(JSON.stringify(value));
}

const OTHER_PROGRAM_ID = new PublicKey(
  "BPFLoaderUpgradeab1e11111111111111111111111"
);

// Zeros compress about 1000:1, so this is a ~17 KB account that expands past
// the default limit.
const BOMB_SIZE = DEFAULT_MAX_IDL_DATA_SIZE + 1024 * 1024;

/** Build a Program Metadata account that stores `data` as the IDL. */
function metadataAccount(
  data: Buffer,
  compression: MetadataCompression,
  encoding = MetadataEncoding.Utf8
): Buffer {
  const header = Buffer.alloc(1 + 32 + 32 + 1 + 1 + 16 + 1 + 1 + 1 + 1);
  let offset = 0;
  header.writeUInt8(2, offset); // metadata account discriminator
  offset += 1;
  OTHER_PROGRAM_ID.toBuffer().copy(header, offset);
  offset += 32 + 32; // program, then a null authority
  offset += 1 + 1; // mutable, canonical
  Buffer.from("idl").copy(header, offset);
  offset += 16;
  header.writeUInt8(encoding, offset);
  offset += 1;
  header.writeUInt8(compression, offset);
  offset += 1;
  header.writeUInt8(FORMAT_JSON, offset);
  offset += 1;
  header.writeUInt8(0, offset); // directly embedded data

  const length = Buffer.alloc(4 + 5);
  length.writeUInt32LE(data.length, 0);
  return Buffer.concat([header, length, data]);
}

function compress(data: Buffer, compression: MetadataCompression): Buffer {
  switch (compression) {
    case MetadataCompression.Gzip:
      return gzipSync(data);
    case MetadataCompression.Zlib:
      return deflateSync(data);
    default:
      return data;
  }
}

function idlAccount(idl: Idl, compression: MetadataCompression): Buffer {
  const json = Buffer.from(JSON.stringify(idl));
  return metadataAccount(compress(json, compression), compression);
}

const bombs = new Map<MetadataCompression, Buffer>();
function bomb(compression: MetadataCompression): Buffer {
  if (!bombs.has(compression)) {
    bombs.set(compression, compress(Buffer.alloc(BOMB_SIZE), compression));
  }
  return bombs.get(compression)!;
}

function bombAccount(
  compression: MetadataCompression,
  encoding = MetadataEncoding.Utf8
): Buffer {
  return metadataAccount(bomb(compression), compression, encoding);
}

/** A provider that serves the given accounts and nothing else. */
function mockProvider(
  accounts: Map<string, { owner: PublicKey; data: Buffer }>
) {
  return {
    connection: {
      getAccountInfo: jest.fn(
        async (address: PublicKey) => accounts.get(address.toBase58()) ?? null
      ),
    },
  };
}

const otherIdl: Idl = {
  address: OTHER_PROGRAM_ID.toBase58(),
  metadata: { name: "other", version: "0.1.0", spec: "0.1.0" },
  instructions: [],
  accounts: [{ name: "Data", discriminator: [1, 2, 3, 4, 5, 6, 7, 8] }],
  types: [
    {
      name: "Data",
      type: { kind: "struct", fields: [{ name: "authority", type: "pubkey" }] },
    },
  ],
};

const compressions: [string, MetadataCompression][] = [
  ["gzip", MetadataCompression.Gzip],
  ["zlib", MetadataCompression.Zlib],
];
const allCompressions: [string, MetadataCompression][] = [
  ["none", MetadataCompression.None],
  ...compressions,
];

describe("decodeIdlAccount", () => {
  afterEach(() => jest.restoreAllMocks());

  it.each(allCompressions)("decodes a normal %s IDL", (_, compression) => {
    expect(decodeIdlAccount(idlAccount(otherIdl, compression))).toEqual(
      otherIdl
    );
  });

  it.each(compressions)(
    "stops inflating a %s IDL at the default limit",
    (_, compression) => {
      // Count the bytes pako actually keeps, and check it never finishes the
      // stream, to show the full output is never built.
      let kept = 0;
      const onData = Inflate.prototype.onData;
      jest
        .spyOn(Inflate.prototype, "onData")
        .mockImplementation(function (this: Inflate, chunk) {
          kept += chunk.length;
          return onData.call(this, chunk);
        });
      const onEnd = jest.spyOn(Inflate.prototype, "onEnd");

      expect(() => decodeIdlAccount(bombAccount(compression))).toThrow(
        `Decompressed IDL data exceeds the maximum of ${DEFAULT_MAX_IDL_DATA_SIZE} bytes`
      );
      // pako emits output in 64 KiB chunks.
      expect(kept).toBeGreaterThan(DEFAULT_MAX_IDL_DATA_SIZE - 64 * 1024);
      expect(kept).toBeLessThanOrEqual(DEFAULT_MAX_IDL_DATA_SIZE);
      expect(onEnd).not.toHaveBeenCalled();
    }
  );

  it.each(allCompressions)(
    "respects a custom limit for a %s IDL",
    (_, compression) => {
      const data = idlAccount(otherIdl, compression);
      const size = JSON.stringify(otherIdl).length;

      expect(decodeIdlAccount(data, { maxDataSize: size })).toEqual(otherIdl);
      expect(() => decodeIdlAccount(data, { maxDataSize: size - 1 })).toThrow(
        `exceeds the maximum of ${size - 1} bytes`
      );
    }
  );

  it("counts the limit across concatenated gzip members", () => {
    const json = Buffer.from(JSON.stringify(otherIdl));
    const data = metadataAccount(
      Buffer.concat([gzipSync(json), gzipSync(json)]),
      MetadataCompression.Gzip
    );

    expect(() =>
      decodeIdlAccount(data, { maxDataSize: json.length + 1 })
    ).toThrow(`exceeds the maximum of ${json.length + 1} bytes`);
  });

  it.each([NaN, -1])("rejects an invalid limit of %s", (maxDataSize) => {
    const data = idlAccount(otherIdl, MetadataCompression.Zlib);

    expect(() => decodeIdlAccount(data, { maxDataSize })).toThrow(
      `Invalid maximum IDL data size: ${maxDataSize}`
    );
  });

  it.each([
    ["hex", MetadataEncoding.None],
    ["base58", MetadataEncoding.Base58],
    ["base64", MetadataEncoding.Base64],
  ])("rejects a %s encoded IDL before decompressing it", (_, encoding) => {
    const data = bombAccount(MetadataCompression.Zlib, encoding);

    expect(() => decodeIdlAccount(data)).toThrow(
      `IDL has encoding '${encoding}', only UTF-8 JSON IDLs`
    );
  });

  it("rejects truncated compressed data", () => {
    const full = deflateSync(Buffer.from(JSON.stringify(otherIdl)));
    const data = metadataAccount(
      full.subarray(0, full.length - 4),
      MetadataCompression.Zlib
    );

    expect(() => decodeIdlAccount(data)).toThrow(
      "Failed to decompress IDL data: unexpected end of data"
    );
  });

  it("rejects corrupt compressed data", () => {
    const data = metadataAccount(
      Buffer.from("not compressed"),
      MetadataCompression.Zlib
    );

    expect(() => decodeIdlAccount(data)).toThrow(
      "Failed to decompress IDL data"
    );
  });
});

describe("Program.fetchIdl", () => {
  it("fetches a normal IDL", async () => {
    const provider = mockProvider(
      new Map([
        [
          idlAddress(OTHER_PROGRAM_ID).toBase58(),
          {
            owner: OTHER_PROGRAM_ID,
            data: idlAccount(otherIdl, MetadataCompression.Zlib),
          },
        ],
      ])
    );

    await expect(
      Program.fetchIdl(OTHER_PROGRAM_ID, provider as any)
    ).resolves.toEqual(otherIdl);
  });

  it("rejects an IDL that decompresses past the limit", async () => {
    const provider = mockProvider(
      new Map([
        [
          idlAddress(OTHER_PROGRAM_ID).toBase58(),
          {
            owner: OTHER_PROGRAM_ID,
            data: bombAccount(MetadataCompression.Zlib),
          },
        ],
      ])
    );

    await expect(
      Program.fetchIdl(OTHER_PROGRAM_ID, provider as any)
    ).rejects.toThrow(
      `Decompressed IDL data exceeds the maximum of ${DEFAULT_MAX_IDL_DATA_SIZE} bytes`
    );
  });

  it("passes the size limit through", async () => {
    const provider = mockProvider(
      new Map([
        [
          idlAddress(OTHER_PROGRAM_ID).toBase58(),
          {
            owner: OTHER_PROGRAM_ID,
            data: idlAccount(otherIdl, MetadataCompression.Zlib),
          },
        ],
      ])
    );

    await expect(
      Program.fetchIdl(OTHER_PROGRAM_ID, provider as any, { maxDataSize: 16 })
    ).rejects.toThrow("Decompressed IDL data exceeds the maximum of 16 bytes");
  });
});

// The accounts resolver fetches the IDL of whichever program owns an account
// used in a PDA seed, so that program's metadata authority controls what the
// client decompresses.
describe("Accounts resolution with a foreign account owner", () => {
  const programId = new PublicKey(
    "Test111111111111111111111111111111111111111"
  );
  const dataAddress = new PublicKey(
    "SysvarC1ock11111111111111111111111111111111"
  );
  const authority = new PublicKey(
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
  );
  const idl: Idl = {
    address: programId.toBase58(),
    metadata: { name: "test", version: "0.1.0", spec: "0.1.0" },
    instructions: [
      {
        name: "doThing",
        discriminator: [0, 0, 0, 0, 0, 0, 0, 0],
        args: [],
        accounts: [
          { name: "data" },
          {
            name: "pda",
            pda: {
              seeds: [
                { kind: "account", path: "data.authority", account: "Data" },
              ],
            },
          },
        ],
      },
    ],
    types: otherIdl.types,
  };
  const [expectedPda] = PublicKey.findProgramAddressSync(
    [authority.toBuffer()],
    programId
  );

  function providerWithOwnerIdl(idlData: Buffer) {
    return mockProvider(
      new Map([
        [
          dataAddress.toBase58(),
          {
            owner: OTHER_PROGRAM_ID,
            data: Buffer.concat([
              Buffer.from([1, 2, 3, 4, 5, 6, 7, 8]),
              authority.toBuffer(),
            ]),
          },
        ],
        [
          idlAddress(OTHER_PROGRAM_ID).toBase58(),
          { owner: OTHER_PROGRAM_ID, data: idlData },
        ],
      ])
    );
  }

  it("resolves a seed from an account whose owner has a normal IDL", async () => {
    const provider = providerWithOwnerIdl(
      idlAccount(otherIdl, MetadataCompression.Zlib)
    );
    const program = new Program(idl, provider as any);

    const pubkeys = await program.methods
      .doThing()
      .accountsPartial({ data: dataAddress })
      .pubkeys();

    expect(pubkeys.pda).toEqual(expectedPda);
  });

  it("fails with an error when the owner's IDL decompresses past the limit", async () => {
    const provider = providerWithOwnerIdl(
      bombAccount(MetadataCompression.Gzip)
    );
    const program = new Program(idl, provider as any);

    await expect(
      program.methods
        .doThing()
        .accountsPartial({ data: dataAddress })
        .instruction()
    ).rejects.toThrow(
      "`pda`: Decompressed IDL data exceeds the maximum of " +
        `${DEFAULT_MAX_IDL_DATA_SIZE} bytes`
    );
  });

  it("does not fetch any IDL when accountsStrict is given the PDA", async () => {
    const provider = providerWithOwnerIdl(
      bombAccount(MetadataCompression.Gzip)
    );
    const program = new Program(idl, provider as any);

    const ix = await program.methods
      .doThing()
      .accountsStrict({ data: dataAddress, pda: expectedPda })
      .instruction();

    expect(ix.keys.map((meta) => meta.pubkey)).toEqual([
      dataAddress,
      expectedPda,
    ]);
    expect(provider.connection.getAccountInfo).not.toHaveBeenCalled();
  });
});
