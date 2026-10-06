import {
  getBase64Decoder,
  getStructCodec,
  getU32Codec,
  getU64Codec,
} from "@solana/kit";
import { utils } from "../src";
import {
  getAnchorAddressCodec,
  getAnchorOptionCodec,
  getRustEnumCodec,
} from "../src/coder/borsh/codecs";
import { mockProvider, randomAddress } from "./helpers/mock-provider";

describe("utils.registry", () => {
  const loaderState = getRustEnumCodec(
    [
      ["uninitialized", getStructCodec([])],
      [
        "buffer",
        getStructCodec([
          ["authorityAddress", getAnchorOptionCodec(getAnchorAddressCodec())],
        ]),
      ],
      [
        "program",
        getStructCodec([["programdataAddress", getAnchorAddressCodec()]]),
      ],
      [
        "programData",
        getStructCodec([
          ["slot", getU64Codec()],
          [
            "upgradeAuthorityAddress",
            getAnchorOptionCodec(getAnchorAddressCodec()),
          ],
        ]),
      ],
    ],
    getU32Codec()
  );

  it("fetches the program data through the Kit rpc", async () => {
    const programAddress = randomAddress();
    const programDataAddress = randomAddress();
    const authority = randomAddress();
    const accounts: Record<string, Uint8Array> = {
      [programAddress]: new Uint8Array(
        loaderState.encode({
          program: { programdataAddress: programDataAddress },
        })
      ),
      [programDataAddress]: new Uint8Array([
        ...loaderState.encode({
          programData: { slot: 7, upgradeAuthorityAddress: authority },
        }),
        // Trailing program bytes, ignored by the decoder.
        ...new Uint8Array(3),
      ]),
    };
    const { provider, requests } = mockProvider({
      getAccountInfo: (request) => {
        const data = accounts[request.params[0] as string];
        return {
          context: { slot: 1 },
          value: {
            data: [getBase64Decoder().decode(data), "base64"],
            executable: false,
            lamports: 1,
            owner: "BPFLoaderUpgradeab1e11111111111111111111111",
            rentEpoch: 0,
            space: data.length,
          },
        };
      },
    });

    const programData = await utils.registry.fetchData(
      provider.rpc,
      programAddress
    );

    expect(programData).toEqual({
      slot: 7n,
      upgradeAuthorityAddress: authority,
    });
    expect(
      requests
        .filter((r) => r.method === "getAccountInfo")
        .map((r) => r.params[0])
    ).toEqual([programAddress, programDataAddress]);
  });

  it("fails clearly when the program data account does not exist", async () => {
    const programAddress = randomAddress();
    const programDataAddress = randomAddress();
    const programData = new Uint8Array(
      loaderState.encode({
        program: { programdataAddress: programDataAddress },
      })
    );
    const { provider, requests } = mockProvider({
      getAccountInfo: (request) => ({
        context: { slot: 1 },
        value:
          request.params[0] === programAddress
            ? {
                data: [getBase64Decoder().decode(programData), "base64"],
                executable: false,
                lamports: 1,
                owner: "BPFLoaderUpgradeab1e11111111111111111111111",
                rentEpoch: 0,
                space: programData.length,
              }
            : null,
      }),
    });

    await expect(
      utils.registry.fetchData(provider.rpc, programAddress)
    ).rejects.toThrow("program data account not found");
    // The walk stopped after the second hop.
    expect(requests.filter((r) => r.method === "getAccountInfo")).toHaveLength(
      2
    );
  });

  it("fails clearly when the program does not exist", async () => {
    const { provider } = mockProvider({
      getAccountInfo: () => ({ context: { slot: 1 }, value: null }),
    });

    await expect(
      utils.registry.fetchData(provider.rpc, randomAddress())
    ).rejects.toThrow("program account not found");
  });
});
