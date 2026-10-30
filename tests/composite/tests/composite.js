const { assert } = require("chai");
const anchor = require("@anchor-lang/core");

describe("composite", () => {
  const provider = anchor.AnchorProvider.local();

  // Configure the client to use the local cluster.
  anchor.setProvider(provider);

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

    it("nested structs deserialize the same args as the parent", async () => {
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
      // `value_b` is only checked by the nested `BarWithArgs` struct.
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

      // `value_a` is checked by the nested `FooWithArgs` struct, which only
      // declares a prefix of the parent's args.
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

      // Nothing was written by the rejected calls.
      const a = await program.account.dummyA.fetch(dummyA.publicKey);
      const b = await program.account.dummyB.fetch(dummyB.publicKey);
      assert.isTrue(a.data.eq(new anchor.BN(111)));
      assert.isTrue(b.data.eq(new anchor.BN(222)));
    });
  });
});
