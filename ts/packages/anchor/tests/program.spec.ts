import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import * as anchor from "../src";
import { Idl, MethodPubkeys, Program } from "../src";

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

describe("MethodPubkeys", () => {
  type TestIdl = {
    address: "Test111111111111111111111111111111111111111";
    metadata: { name: "test"; version: "0.1.0"; spec: "0.1.0" };
    instructions: [
      {
        name: "myMethod";
        discriminator: [0, 1, 2, 3, 4, 5, 6, 7];
        accounts: [
          { name: "authority"; writable: true; signer: true },
          { name: "dataAccount"; writable: true },
          {
            name: "nestedGroup";
            accounts: [
              { name: "childA"; writable: false },
              { name: "childB"; writable: true }
            ];
          }
        ];
        args: [];
      },
      {
        name: "otherMethod";
        discriminator: [8, 9, 10, 11, 12, 13, 14, 15];
        accounts: [{ name: "payer"; signer: true }];
        args: [];
      }
    ];
  };

  it("extracts resolved account public keys matching rpcAndKeys pubkeys type", () => {
    type ExpectedPubkeys = Awaited<
      ReturnType<
        ReturnType<Program<TestIdl>["methods"]["myMethod"]>["rpcAndKeys"]
      >
    >["pubkeys"];

    type ActualPubkeys = anchor.MethodPubkeys<TestIdl, "myMethod">;

    // Compile-time type check: ActualPubkeys and ExpectedPubkeys must be mutually assignable
    type AssertEqual<T, U> = [T] extends [U]
      ? [U] extends [T]
        ? true
        : false
      : false;
    const typeEqualityCheck: AssertEqual<ActualPubkeys, ExpectedPubkeys> = true;
    expect(typeEqualityCheck).toBe(true);

    const kAuthority = Keypair.generate().publicKey;
    const kDataAccount = Keypair.generate().publicKey;
    const kChildA = Keypair.generate().publicKey;
    const kChildB = Keypair.generate().publicKey;
    const kPayer = Keypair.generate().publicKey;

    const pubkeys: anchor.MethodPubkeys<TestIdl, "myMethod"> = {
      authority: kAuthority,
      dataAccount: kDataAccount,
      nestedGroup: {
        childA: kChildA,
        childB: kChildB,
      },
    };

    expect(pubkeys.authority.equals(kAuthority)).toBe(true);
    expect(pubkeys.dataAccount.equals(kDataAccount)).toBe(true);
    expect(pubkeys.nestedGroup.childA.equals(kChildA)).toBe(true);
    expect(pubkeys.nestedGroup.childB.equals(kChildB)).toBe(true);

    const otherPubkeys: MethodPubkeys<TestIdl, "otherMethod"> = {
      payer: kPayer,
    };
    expect(otherPubkeys.payer.equals(kPayer)).toBe(true);
  });
});
