import * as anchor from "@anchor-lang/core";
import { AnchorProvider, Program } from "@anchor-lang/core";
import { PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { RelationsDerivation } from "../target/types/relations_derivation";

describe("typescript", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .RelationsDerivation as Program<RelationsDerivation>;
  const provider = anchor.getProvider() as AnchorProvider;

  it("Inits the base account", async () => {
    await program.methods
      .initBase()
      .accounts({
        myAccount: provider.wallet.publicKey,
      })
      .rpc();
  });

  it("Derives relationss", async () => {
    const tx = await program.methods.testRelation().accounts({
      nested: {
        account: (
          await PublicKey.findProgramAddress(
            [Buffer.from("seed", "utf-8")],
            program.programId
          )
        )[0],
      },
    });

    await tx.instruction();
    const keys = await tx.pubkeys();

    expect(keys.myAccount!.equals(provider.wallet.publicKey)).is.true;

    await tx.rpc();
  });

  it("Can use `address` constraint", () => {
    const ix = program.idl.instructions.find(
      (ix) => ix.name === "testAddress"
    )!;
    expect(ix.accounts.find((acc) => acc.name === "constant")!.address).to.not
      .be.undefined;
    expect(ix.accounts.find((acc) => acc.name === "constFn")!.address).to.not.be
      .undefined;
  });
});
