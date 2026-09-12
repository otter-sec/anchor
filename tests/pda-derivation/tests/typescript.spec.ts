import * as anchor from "@anchor-lang/core";
import BN from "bn.js";
import { Keypair, PublicKey } from "@solana/web3.js";
import { Program } from "@anchor-lang/core";
import { PdaDerivation } from "../target/types/pda_derivation";
import { expect } from "chai";
const encode = anchor.utils.bytes.utf8.encode;

describe("typescript", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.PdaDerivation as Program<PdaDerivation>;
  const base = Keypair.generate();
  const dataKey = Keypair.generate();
  const data = new BN(1);
  const another = Keypair.generate();
  const anotherData = new BN(2);
  const seedA = 4;

  it("Inits the base account", async () => {
    await program.methods
      .initBase(data, dataKey.publicKey)
      .accounts({
        base: base.publicKey,
      })
      .signers([base])
      .rpc();

    await program.methods
      .initAnother(anotherData)
      .accounts({
        base: another.publicKey,
      })
      .signers([another])
      .rpc();
  });

  it("Inits the derived accounts", async () => {
    const MY_SEED = "hi";
    const MY_SEED_STR = "hi";
    const MY_SEED_U8 = 1;
    const MY_SEED_U32 = 2;
    const MY_SEED_U64 = 3;
    const expectedPDAKey = PublicKey.findProgramAddressSync(
      [
        Buffer.from([seedA]),
        encode("another-seed"),
        encode("test"),
        base.publicKey.toBuffer(),
        base.publicKey.toBuffer(),
        encode(MY_SEED),
        encode(MY_SEED_STR),
        Buffer.from([MY_SEED_U8]),
        new anchor.BN(MY_SEED_U32).toArrayLike(Buffer, "le", 4),
        new anchor.BN(MY_SEED_U64).toArrayLike(Buffer, "le", 8),
        new anchor.BN(data).toArrayLike(Buffer, "le", 8),
        dataKey.publicKey.toBuffer(),
        new anchor.BN(anotherData).toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    )[0];

    const tx = program.methods.initMyAccount(seedA).accountsPartial({
      base: base.publicKey,
      base2: base.publicKey,
      anotherBase: another.publicKey,
    });

    const keys = await tx.pubkeys();
    expect(keys.account!.equals(expectedPDAKey)).is.true;

    await tx.rpc();

    const actualData = (await program.account.myAccount.fetch(expectedPDAKey))
      .data;
    expect(actualData.toNumber()).is.equal(1337);
  });

  it("should allow custom resolvers", async () => {
    let called = false;
    const customProgram = new Program<PdaDerivation>(
      program.idl,
      program.provider,
      program.coder,
      (instruction) => {
        if (instruction.name === "initMyAccount") {
          return async ({ accounts }) => {
            called = true;
            return { accounts, resolved: 0 };
          };
        }
      }
    );
    await customProgram.methods
      .initMyAccount(seedA)
      .accountsPartial({
        base: base.publicKey,
        base2: base.publicKey,
        anotherBase: another.publicKey,
      })
      .pubkeys();

    expect(called).is.true;
  });

  it("Can use constant seed ref", async () => {
    await program.methods.testSeedConstant().rpc();
  });

  it("Can resolve associated token accounts", async () => {
    const mintKp = anchor.web3.Keypair.generate();
    await program.methods
      .associatedTokenResolution()
      .accounts({ mint: mintKp.publicKey })
      .signers([mintKp])
      .rpc();
  });

  // TODO: Support more expressions in the IDL e.g. math operations?
  it("Can use unsupported expressions", () => {
    // Compilation test to fix issues like https://github.com/coral-xyz/anchor/issues/2933
  });

  it("Includes the unresolved accounts if resolution fails", async () => {
    try {
      // `unknown` account is required for account resolution to work, but it's
      // intentionally not provided to test the error message
      await program.methods.resolutionError().rpc();
      throw new Error("Should throw due to account resolution failure!");
    } catch (e) {
      expect(e.message).to.equal(
        "Reached maximum depth for account resolution. Unresolved accounts: `pda`, `anotherPda`"
      );
    }
  });

  it("Skips resolution if `program::seeds` expression is not supported", async () => {
    const acc = program.idl.instructions
      .find((ix) => ix.name === "unsupportedProgramSeed")!
      .accounts.find((acc) => acc.name === "pda")!;
    // @ts-expect-error
    expect(acc.pda).to.be.undefined;
  });

  it("Can resolve call expressions with no arguments", async () => {
    await program.methods.callExprWithNoArgs().rpc();
  });

  it("Can use `Pubkey` constants with `seeds::program`", async () => {
    await program.methods.pubkeyConst().rpc();
  });

  it("Can use accounts with `seeds::program`", async () => {
    await program.methods.seedsProgramAccount().rpc();
  });

  it("Can use arguments with `seeds::program`", async () => {
    await program.methods.seedsProgramArg(anchor.web3.PublicKey.default).rpc();
  });
});
