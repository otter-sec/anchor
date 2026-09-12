import * as anchor from "@anchor-lang/core";
import { AnchorError, Program } from "@anchor-lang/core";
import { InitIfNeeded } from "../../target/types/init_if_needed";
import { SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";

describe("init-if-needed", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.InitIfNeeded as Program<InitIfNeeded>;

  it("init_if_needed should reject a CLOSED discriminator if init is NOT NEEDED", async () => {
    const account = anchor.web3.Keypair.generate();

    await program.methods
      .initialize(1)
      .accounts({
        acc: account.publicKey,
      })
      .signers([account])
      .rpc();

    const oldState = await program.account.myData.fetch(account.publicKey);
    expect(oldState.val.toNumber()).to.equal(1000);

    // This initialize call should fail because the account has the account discriminator
    // set to the CLOSED one
    try {
      await program.methods
        .initialize(5)
        .accounts({
          acc: account.publicKey,
        })
        .signers([account])
        .preInstructions([
          await program.methods
            .close()
            .accounts({
              acc: account.publicKey,
              receiver: provider.wallet.publicKey,
            })
            .instruction(),
          SystemProgram.transfer({
            fromPubkey: provider.wallet.publicKey,
            toPubkey: account.publicKey,
            lamports: 1 * LAMPORTS_PER_SOL,
          }),
        ])
        .rpc();
    } catch (_err) {
      expect(_err).to.be.instanceOf(AnchorError);
      const err: AnchorError = _err;
      expect(err.error.errorCode.code).to.equal("AccountDiscriminatorMismatch");
    }
  });

  it("init_if_needed should reject a discriminator of a different account if init is NOT NEEDED", async () => {
    const account = anchor.web3.Keypair.generate();
    console.log("account: ", account.publicKey.toBase58());
    const otherAccount = anchor.web3.Keypair.generate();
    console.log("otherAccount: ", otherAccount.publicKey.toBase58());

    await program.methods
      .initialize(1)
      .accounts({
        acc: account.publicKey,
      })
      .signers([account])
      .rpc();

    const oldState = await program.account.myData.fetch(account.publicKey);
    expect(oldState.val.toNumber()).to.equal(1000);

    await program.methods
      .secondInitialize(1)
      .accounts({
        acc: otherAccount.publicKey,
      })
      .signers([otherAccount])
      .rpc();

    const secondState = await program.account.otherData.fetch(
      otherAccount.publicKey
    );
    expect(secondState.otherVal.toNumber()).to.equal(2000);

    try {
      await program.methods
        .initialize(3)
        .accounts({
          acc: otherAccount.publicKey,
        })
        .signers([otherAccount])
        .rpc();
    } catch (_err) {
      expect(_err).to.be.instanceOf(AnchorError);
      const err: AnchorError = _err;
      expect(err.error.errorCode.code).to.equal("AccountDiscriminatorMismatch");
    }
  });
});
