const anchor = require("@anchor-lang/core");
const { assert } = require("chai");

describe("sysvars", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.local());
  const program = anchor.workspace.Sysvars;

  it("Is initialized!", async () => {
    const tx = await program.methods
      .sysvars()
      .accounts({
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        stakeHistory: anchor.web3.SYSVAR_STAKE_HISTORY_PUBKEY,
      })
      .rpc();
    console.log("Your transaction signature", tx);
  });

  it("Fails when the wrong pubkeys are provided", async () => {
    try {
      await program.methods
        .sysvars()
        .accounts({
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          stakeHistory: anchor.web3.SYSVAR_REWARDS_PUBKEY,
        })
        .rpc();
      assert.ok(false);
    } catch (err) {
      const errMsg = "The given public key does not match the required sysvar";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 3015);
    }
  });
});
