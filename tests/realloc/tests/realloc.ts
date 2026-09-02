import * as anchor from "@anchor-lang/core";
import { AnchorError, Program } from "@anchor-lang/core";
import { assert } from "chai";
import { Realloc } from "../target/types/realloc";

describe("realloc", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.realloc as Program<Realloc>;
  const authority = (program.provider as any).wallet
    .payer as anchor.web3.Keypair;

  let sample: anchor.web3.PublicKey;
  let payer: anchor.web3.PublicKey;

  before(async () => {
    [sample] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("sample")],
      program.programId
    );
    [payer] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("payer")],
      program.programId
    );
  });

  it("initialized", async () => {
    await program.methods
      .initialize()
      .accounts({ authority: authority.publicKey, sample })
      .rpc();

    const samples = await program.account.sample.all();
    assert.lengthOf(samples, 1);
    assert.lengthOf(samples[0].account.data, 1);
  });

  it("fails if delta bytes exceeds permitted limit", async () => {
    try {
      await program.methods
        .realloc(10250)
        .accounts({ authority: authority.publicKey, sample })
        .rpc();
      assert.ok(false);
    } catch (e) {
      assert.isTrue(e instanceof AnchorError);
      const err: AnchorError = e;
      const errMsg =
        "The account reallocation exceeds the MAX_PERMITTED_DATA_INCREASE limit";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 3016);
    }
  });

  it("initializes boxed realloc payer", async () => {
    await program.methods
      .initializeBoxPayer()
      .accounts({ authority: authority.publicKey, payer })
      .rpc();

    const payerAccount = await program.account.sample.fetch(payer);
    assert.lengthOf(payerAccount.data, 1);
  });

  it("realloc additive", async () => {
    await program.methods
      .realloc(5)
      .accounts({ authority: authority.publicKey, sample })
      .rpc();

    const s = await program.account.sample.fetch(sample);
    assert.lengthOf(s.data, 5);
  });

  it("realloc subtractive", async () => {
    await program.methods
      .realloc(1)
      .accounts({ authority: authority.publicKey, sample })
      .rpc();

    const s = await program.account.sample.fetch(sample);
    assert.lengthOf(s.data, 1);
  });

  it("realloc subtractive with boxed payer", async () => {
    await program.methods
      .realloc(5)
      .accounts({ authority: authority.publicKey, sample })
      .rpc();

    const beforeSample = await program.provider.connection.getAccountInfo(
      sample
    );
    const beforePayer = await program.provider.connection.getAccountInfo(payer);
    assert.isNotNull(beforeSample);
    assert.isNotNull(beforePayer);

    await program.methods
      .reallocBoxPayer(1)
      .accounts({
        authority: authority.publicKey,
        sample,
        payer,
      })
      .rpc();

    const afterSample = await program.provider.connection.getAccountInfo(
      sample
    );
    const afterPayer = await program.provider.connection.getAccountInfo(payer);
    assert.isNotNull(afterSample);
    assert.isNotNull(afterPayer);
    assert.isBelow(afterSample!.lamports, beforeSample!.lamports);
    assert.isAbove(afterPayer!.lamports, beforePayer!.lamports);

    const s = await program.account.sample.fetch(sample);
    assert.lengthOf(s.data, 1);
  });

  it("fails with duplicate account reallocations", async () => {
    try {
      await program.methods
        .realloc2(1000)
        .accounts({
          authority: authority.publicKey,
          sample1: sample,
          sample2: sample,
        })
        .rpc();
    } catch (e) {
      assert.isTrue(e instanceof AnchorError);
      const err: AnchorError = e;
      const errMsg =
        "The account was duplicated for more than one reallocation";
      assert.strictEqual(err.error.errorMessage, errMsg);
      assert.strictEqual(err.error.errorCode.number, 3017);
    }
  });
});
