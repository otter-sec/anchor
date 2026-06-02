const assert = require("assert");
const anchor = require("@anchor-lang/core");

describe("basic-4", () => {
  const provider = anchor.AnchorProvider.local();

  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  const program = anchor.workspace.Basic4,
    counterSeed = anchor.utils.bytes.utf8.encode("counter");

  let counterPubkey;

  before(async () => {
    [counterPubkey] = await anchor.web3.PublicKey.findProgramAddress(
      [counterSeed],
      program.programId
    );
  });

  it("Is runs the constructor", async () => {
    // Initialize the program's state struct.
    await program.methods
      .initialize()
      .accounts({
        // counter: counterPubkey,
        /* 
        A accounts whose seeds are fully declared in the IDL 
        (e.g. counter has pda.seeds = [{ kind: "const", value: [...] }])
        client derives the address at call time, no need to pass it.
        */
        authority: provider.wallet.publicKey,
        // systemProgram: anchor.web3.SystemProgram.programId,
        /*
         Known system programs whose address is fixed in the IDL
          (e.g. systemProgram → "11111111111111111111111111111111",
         tokenProgram  → resolved from the token interface constraint) client fills these in automatically.

        */
      })
      .rpc();

    // Fetch the state struct from the network.
    const counterAccount = await program.account.counter.fetch(counterPubkey);

    assert.ok(counterAccount.count.eq(new anchor.BN(0)));
  });

  it("Executes a method on the program", async () => {
    await program.methods
      .increment()
      .accounts({
        authority: provider.wallet.publicKey,
      })
      .rpc();

    const counterAccount = await program.account.counter.fetch(counterPubkey);
    assert.ok(counterAccount.count.eq(new anchor.BN(1)));
  });
});
