const { assert } = require("chai");
const anchor = require("@anchor-lang/core");

describe("composite", () => {
  const provider = anchor.AnchorProvider.local(process.env.ANCHOR_PROVIDER_URL);

  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

  async function transactionLogs(signature) {
    // RPC transaction history can lag behind confirmation on a local validator.
    for (let attempt = 0; attempt < 40; attempt++) {
      const tx = await provider.connection.getTransaction(signature, {
        commitment: "confirmed",
      });
      if (tx) return tx.meta.logMessages;
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    assert.fail(`transaction history unavailable for ${signature}`);
  }

  it("Is initialized!", async () => {
    const program = anchor.workspace.composite;

    const dummyA = anchor.web3.Keypair.generate();
    const dummyB = anchor.web3.Keypair.generate();

    const tx = await program.rpc.initialize({
      accounts: {
        dummyA: dummyA.publicKey,
        dummyB: dummyB.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
      signers: [dummyA, dummyB],
      instructions: [
        await program.account.dummyA.createInstruction(dummyA),
        await program.account.dummyB.createInstruction(dummyB),
      ],
    });

    await program.rpc.compositeUpdate(
      new anchor.BN(1234),
      new anchor.BN(4321),
      {
        accounts: {
          foo: {
            dummyA: dummyA.publicKey,
          },
          bar: {
            dummyB: dummyB.publicKey,
          },
        },
      }
    );

    const dummyAAccount = await program.account.dummyA.fetch(dummyA.publicKey);
    const dummyBAccount = await program.account.dummyB.fetch(dummyB.publicKey);

    assert.isTrue(dummyAAccount.data.eq(new anchor.BN(1234)));
    assert.isTrue(dummyBAccount.data.eq(new anchor.BN(4321)));
  });

  describe("composite structs with #[instruction(..)] args (#2942)", () => {
    const program = anchor.workspace.composite;
    const dummyA = anchor.web3.Keypair.generate();
    const dummyB = anchor.web3.Keypair.generate();

    before(async () => {
      await program.methods
        .initialize()
        .accounts({
          dummyA: dummyA.publicKey,
          dummyB: dummyB.publicKey,
        })
        .preInstructions([
          await program.account.dummyA.createInstruction(dummyA),
          await program.account.dummyB.createInstruction(dummyB),
        ])
        .signers([dummyA, dummyB])
        .rpc();
    });

    it("keeps identical local argument names positional", async () => {
      const signature = await program.methods
        .legacyLocalName(new anchor.BN(7), 9)
        .accounts({ child: { dummyA: dummyA.publicKey } })
        .rpc();
      assert.include(
        await transactionLogs(signature),
        "Program log: legacy args: 7, 9"
      );
      const account = await program.account.dummyA.fetch(dummyA.publicKey);
      assert.isTrue(account.data.eq(new anchor.BN(9)));

      try {
        await program.methods
          .legacyLocalName(new anchor.BN(9), 8)
          .accounts({ child: { dummyA: dummyA.publicKey } })
          .rpc();
        assert.fail("expected the child to reject its own argument");
      } catch (err) {
        assert.instanceOf(err, anchor.AnchorError);
        assert.strictEqual(err.error.errorCode.code, "WrongLocalValue");
        assert.strictEqual(err.error.origin, "dummy_a");
      }
      const unchanged = await program.account.dummyA.fetch(dummyA.publicKey);
      assert.isTrue(unchanged.data.eq(new anchor.BN(9)));
    });

    it("uses an explicit argument after a variable prefix for PDA seeds", async () => {
      const seedAddress = (value) =>
        anchor.web3.PublicKey.findProgramAddressSync(
          [
            Buffer.from("selected"),
            new anchor.BN(value).toArrayLike(Buffer, "le", 4),
          ],
          program.programId
        )[0];
      const signature = await program.methods
        .explicitSeed("variable prefix", 42)
        .accounts({
          child: { target: seedAddress(42), dummyA: dummyA.publicKey },
        })
        .rpc();
      assert.include(
        await transactionLogs(signature),
        "Program log: selected args: variable prefix, 42"
      );
      const account = await program.account.dummyA.fetch(dummyA.publicKey);
      assert.isTrue(account.data.eq(new anchor.BN(42)));
      try {
        await program.methods
          .explicitSeed("different prefix", 43)
          .accounts({
            child: { target: seedAddress(42), dummyA: dummyA.publicKey },
          })
          .rpc();
        assert.fail("expected PDA seeds to use the selected argument");
      } catch (err) {
        assert.instanceOf(err, anchor.AnchorError);
        assert.strictEqual(err.error.errorCode.code, "ConstraintSeeds");
        assert.strictEqual(err.error.origin, "target");
      }
      const unchanged = await program.account.dummyA.fetch(dummyA.publicKey);
      assert.isTrue(unchanged.data.eq(new anchor.BN(42)));
    });

    it("nested structs read the args they declare", async () => {
      await program.methods
        .compositeUpdateWithArgs(new anchor.BN(111), new anchor.BN(222))
        .accounts({
          foo: { dummyA: dummyA.publicKey },
          bar: { dummyB: dummyB.publicKey },
        })
        .rpc();

      const a = await program.account.dummyA.fetch(dummyA.publicKey);
      const b = await program.account.dummyB.fetch(dummyB.publicKey);
      assert.isTrue(a.data.eq(new anchor.BN(111)));
      assert.isTrue(b.data.eq(new anchor.BN(222)));
    });

    it("a constraint inside a nested struct sees the real arg value", async () => {
      try {
        await program.methods
          .compositeUpdateWithArgs(new anchor.BN(111), new anchor.BN(0))
          .accounts({
            foo: { dummyA: dummyA.publicKey },
            bar: { dummyB: dummyB.publicKey },
          })
          .rpc();
        assert.fail("expected the nested constraint to reject value_b == 0");
      } catch (err) {
        assert.instanceOf(err, anchor.AnchorError);
        assert.strictEqual(err.error.errorCode.code, "ZeroValueB");
        assert.strictEqual(err.error.origin, "dummy_b");
      }

      try {
        await program.methods
          .compositeUpdateWithArgs(new anchor.BN(0), new anchor.BN(222))
          .accounts({
            foo: { dummyA: dummyA.publicKey },
            bar: { dummyB: dummyB.publicKey },
          })
          .rpc();
        assert.fail("expected the nested constraint to reject value_a == 0");
      } catch (err) {
        assert.instanceOf(err, anchor.AnchorError);
        assert.strictEqual(err.error.errorCode.code, "ZeroValueA");
        assert.strictEqual(err.error.origin, "dummy_a");
      }

      const a = await program.account.dummyA.fetch(dummyA.publicKey);
      const b = await program.account.dummyB.fetch(dummyB.publicKey);
      assert.isTrue(a.data.eq(new anchor.BN(111)));
      assert.isTrue(b.data.eq(new anchor.BN(222)));
    });
  });
});
