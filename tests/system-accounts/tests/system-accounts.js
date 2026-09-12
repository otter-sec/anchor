const anchor = require("@anchor-lang/core");
const splToken = require("@solana/spl-token");
const { assert } = require("chai");

describe("system_accounts", () => {
  anchor.setProvider(anchor.AnchorProvider.local());
  const program = anchor.workspace.SystemAccounts;
  const authority = program.provider.wallet.payer;
  const wallet = anchor.web3.Keypair.generate();

  it("Is initialized!", async () => {
    const tx = await program.rpc.initialize({
      accounts: {
        authority: authority.publicKey,
        wallet: wallet.publicKey,
      },
      signers: [authority],
    });

    console.log("Your transaction signature", tx);
  });

  it("Emits an AccountNotSystemOwned error", async () => {
    const mint = await splToken.Token.createMint(
      program.provider.connection,
      authority,
      authority.publicKey,
      null,
      9,
      splToken.TOKEN_PROGRAM_ID
    );

    const tokenAccount = await mint.createAssociatedTokenAccount(
      wallet.publicKey
    );

    await mint.mintTo(
      tokenAccount,
      authority.publicKey,
      [],
      1 * anchor.web3.LAMPORTS_PER_SOL
    );

    try {
      await program.rpc.initialize({
        accounts: {
          authority: authority.publicKey,
          wallet: tokenAccount,
        },
        signers: [authority],
      });
      assert.ok(false);
    } catch (err) {
      const errMsg = "The given account is not owned by the system program";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 3011);
    }
  });
});
