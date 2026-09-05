import * as anchor from "@anchor-lang/core";
import assert from "assert";
import BN from "bn.js";

import type { Modularized } from "../target/types/modularized";
import type { Caller } from "../target/types/caller";

describe("modularized-program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program: anchor.Program<Modularized> = anchor.workspace.modularized;
  const caller: anchor.Program<Caller> = anchor.workspace.caller;

  const counterPda = (payer: anchor.web3.PublicKey) =>
    anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("counter"), payer.toBuffer()],
      program.programId
    )[0];

  it("Initializes via a module-qualified Accounts struct", async () => {
    // `counter` is resolved from the IDL seeds, exercising IDL generation
    // for accounts structs in nested modules.
    const { pubkeys, signature } = await program.methods
      .init(new BN(42))
      .rpcAndKeys();
    await provider.connection.confirmTransaction(signature, "confirmed");

    const counter = await program.account.counter.fetch(
      pubkeys.counter as anchor.web3.PublicKey
    );
    assert.strictEqual(counter.count.toNumber(), 42);
  });

  it("Updates via a crate-qualified Accounts struct", async () => {
    await program.methods.update(new BN(43)).rpc();

    const counter = await program.account.counter.fetch(
      counterPda(provider.wallet.publicKey)
    );
    assert.strictEqual(counter.count.toNumber(), 43);
  });

  it("Works with a plain (single-segment) Accounts struct", async () => {
    await program.methods.ping().rpc();
  });

  it("CPIs into an instruction with a module-qualified Accounts struct", async () => {
    const payer = anchor.web3.Keypair.generate();
    const airdrop = await provider.connection.requestAirdrop(
      payer.publicKey,
      anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop, "confirmed");

    const counterAddress = counterPda(payer.publicKey);
    await caller.methods
      .proxyInit(new BN(44))
      .accounts({
        counter: counterAddress,
        payer: payer.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        modularizedProgram: program.programId,
      })
      .signers([payer])
      .rpc();

    const counter = await program.account.counter.fetch(counterAddress);
    assert.strictEqual(counter.count.toNumber(), 44);
  });
});
