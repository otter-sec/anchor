import {
  AccountRole,
  address,
  createNoopSigner,
  Instruction,
} from "@solana/kit";
import TransactionFactory from "../src/program/namespace/transaction";
import InstructionFactory from "../src/program/namespace/instruction";
import { BorshCoder, Idl, Provider } from "../src";

describe("Transaction", () => {
  const programAddress = address("Test111111111111111111111111111111111111111");
  const preIx: Instruction = {
    programAddress: address("11111111111111111111111111111111"),
    accounts: [],
    data: new TextEncoder().encode("pre"),
  };
  const postIx: Instruction = {
    programAddress: address("11111111111111111111111111111111"),
    accounts: [],
    data: new TextEncoder().encode("post"),
  };
  const idl: Idl = {
    address: programAddress,
    metadata: {
      name: "basic_0",
      version: "0.0.0",
      spec: "0.1.0",
    },
    instructions: [
      {
        name: "initialize",
        accounts: [{ name: "authority", signer: true }],
        args: [],
        discriminator: [1, 2, 3, 4, 5, 6, 7, 8],
      },
    ],
  };
  const authority = createNoopSigner(
    address("SysvarRent111111111111111111111111111111111")
  );

  function buildTxFn(provider = {} as Provider) {
    const coder = new BorshCoder(idl);
    const ixItem = InstructionFactory.build(
      idl.instructions[0],
      (ixName, ix) => coder.instruction.encode(ixName, ix),
      programAddress
    );
    return TransactionFactory.build(idl.instructions[0], ixItem, provider);
  }

  it("builds a versioned message with the program instruction", () => {
    const message = buildTxFn()({ accounts: { authority: authority.address } });
    expect(message.version).toBe(1);
    expect(message.instructions).toHaveLength(1);
    expect(message.instructions[0].programAddress).toBe(programAddress);
    expect(message.instructions[0].accounts).toEqual([
      { address: authority.address, role: AccountRole.READONLY_SIGNER },
    ]);
    expect(message.instructions[0].data).toEqual(
      new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8])
    );
    // No fee payer nor lifetime: the provider sets them when sending.
    expect("feePayer" in message).toBe(false);
    expect("lifetimeConstraint" in message).toBe(false);
  });

  it("builds messages of the provider's transaction version", () => {
    const provider = { transactionVersion: "legacy" } as Provider;
    const message = buildTxFn(provider)({
      accounts: { authority: authority.address },
    });
    expect(message.version).toBe("legacy");
  });

  it("prefers the context's transaction version over the provider's", () => {
    const provider = { transactionVersion: "legacy" } as Provider;
    const message = buildTxFn(provider)({
      accounts: { authority: authority.address },
      transactionVersion: 0,
    });
    expect(message.version).toBe(0);
  });

  it("adds pre instructions before the method instruction", () => {
    const message = buildTxFn()({
      accounts: { authority: authority.address },
      preInstructions: [preIx],
    });
    expect(message.instructions).toHaveLength(2);
    expect(message.instructions[0]).toMatchObject(preIx);
  });

  it("adds post instructions after the method instruction", () => {
    const message = buildTxFn()({
      accounts: { authority: authority.address },
      postInstructions: [postIx],
    });
    expect(message.instructions).toHaveLength(2);
    expect(message.instructions[1]).toMatchObject(postIx);
  });

  it("attaches the context signers to the accounts they sign for", () => {
    const message = buildTxFn()({
      accounts: { authority: authority.address },
      signers: [authority],
    });
    expect(message.instructions[0].accounts?.[0]).toMatchObject({
      address: authority.address,
      signer: authority,
    });
  });
});
