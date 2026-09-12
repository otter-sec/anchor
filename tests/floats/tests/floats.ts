import * as anchor from "@anchor-lang/core";
import { Program, getProvider } from "@anchor-lang/core";
import { Keypair, SystemProgram } from "@solana/web3.js";
import { Floats } from "../target/types/floats";
import { assert } from "chai";

describe("floats", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Floats as Program<Floats>;

  it("Creates an account with float data", async () => {
    const accountKeypair = Keypair.generate();

    await program.methods
      .create(1.0, 2.0)
      .accounts({
        account: accountKeypair.publicKey,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([accountKeypair])
      .rpc();

    const account = await program.account.floatDataAccount.fetch(
      accountKeypair.publicKey
    );

    assert.strictEqual(account.dataF32, 1.0);
    assert.strictEqual(account.dataF64, 2.0);
  });

  it("Updates an account with float data", async () => {
    const accountKeypair = Keypair.generate();
    const authorityPublicKey = provider.wallet.publicKey;

    await program.methods
      .create(1.0, 2.0)
      .accounts({
        account: accountKeypair.publicKey,
        authority: authorityPublicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([accountKeypair])
      .rpc();

    let account = await program.account.floatDataAccount.fetch(
      accountKeypair.publicKey
    );

    await program.methods
      .update(3.0, 4.0)
      .accounts({
        account: accountKeypair.publicKey,
        authority: authorityPublicKey,
      })
      .rpc();

    account = await program.account.floatDataAccount.fetch(
      accountKeypair.publicKey
    );

    assert.strictEqual(account.dataF32, 3.0);
    assert.strictEqual(account.dataF64, 4.0);
  });
});
