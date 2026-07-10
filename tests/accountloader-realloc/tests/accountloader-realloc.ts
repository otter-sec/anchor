import * as anchor from "@anchor-lang/core";
import { AnchorError, Program } from "@anchor-lang/core";
import { assert } from "chai";
import { AccountloaderRealloc } from "../target/types/accountloader_realloc";

describe("accountloader-realloc", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .accountloaderRealloc as Program<AccountloaderRealloc>;
  const authority = program.provider.wallet!.payer;

  const DISCRIMINATOR_LEN = 8;

  let data: anchor.web3.PublicKey;
  let legacy: anchor.web3.PublicKey;

  before(async () => {
    [data] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("data")],
      program.programId
    );
    [legacy] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("legacy")],
      program.programId
    );
  });

  it("initializes the zero-copy account at full size", async () => {
    await program.methods
      .initialize()
      .accounts({ authority: authority.publicKey, data })
      .rpc();

    const account = await program.account.data.fetch(data);
    assert.strictEqual(account.value.toString(), "42");
  });

  it("under-sized AccountLoader realloc aborts the tx with AccountDidNotDeserialize (3003) and does not brick", async () => {
    try {
      await program.methods
        .shrink(DISCRIMINATOR_LEN)
        .accounts({ authority: authority.publicKey, data })
        .rpc();
      assert.fail(
        "expected shrink of AccountLoader<Data> below 8 + size_of::<Data>() to abort the tx"
      );
    } catch (e) {
      assert.isTrue(e instanceof AnchorError, `unexpected error: ${e}`);
      const err = e as AnchorError;
      // 3003 = AccountDidNotDeserialize
      assert.strictEqual(
        err.error.errorCode.number,
        3003,
        `expected AccountDidNotDeserialize (3003), got ${err.error.errorCode.number} (${err.error.errorCode.code})`
      );
      assert.strictEqual(err.error.errorCode.code, "AccountDidNotDeserialize");
    }

    // Account body must be unchanged (Solana rolls back state on tx failure).
    const info = await program.provider.connection.getAccountInfo(data);
    assert.isNotNull(info, "account should still exist");
    assert.isAbove(
      info!.data.length,
      DISCRIMINATOR_LEN,
      "account body must NOT have been truncated"
    );

    // `load()` continues to work — value preserved.
    const value = await program.methods.read().accounts({ data }).view();
    assert.strictEqual(value.toString(), "42", "zero-copy value preserved");
  });

  it("accepts AccountLoader realloc at exactly DISCRIMINATOR + size_of::<Data>()", async () => {
    const minSize = 8 + 8 + 64; // disc + value + padding

    // Grow above the minimum first so the next realloc is an actual shrink
    await program.methods
      .shrink(minSize + 20)
      .accounts({ authority: authority.publicKey, data })
      .rpc();

    let info = await program.provider.connection.getAccountInfo(data);
    assert.isNotNull(info);
    assert.strictEqual(info!.data.length, minSize + 20);

    // Oversized account stays readable.
    let value = await program.methods.read().accounts({ data }).view();
    assert.strictEqual(value.toString(), "42");

    // Real shrink down to the exact minimum must succeed.
    await program.methods
      .shrink(minSize)
      .accounts({ authority: authority.publicKey, data })
      .rpc();

    info = await program.provider.connection.getAccountInfo(data);
    assert.isNotNull(info);
    assert.strictEqual(info!.data.length, minSize);

    // Value still readable after the shrink.
    value = await program.methods.read().accounts({ data }).view();
    assert.strictEqual(value.toString(), "42");
  });

  it("migrates a legacy zero-copy account by growing it with realloc", async () => {
    const V1_LEN = 8 + 8; // disc + value
    const V2_LEN = 8 + 16; // disc + value + extra

    // Account as an older program version left it: v1 footprint, valid
    // discriminator.
    await program.methods
      .initializeLegacy()
      .accounts({ authority: authority.publicKey, counter: legacy })
      .rpc();

    let info = await program.provider.connection.getAccountInfo(legacy);
    assert.isNotNull(info);
    assert.strictEqual(info!.data.length, V1_LEN);
    assert.strictEqual(info!.data.readBigUInt64LE(8).toString(), "42");

    // The migrate ix takes the account as AccountLoader<CounterV2> and its
    // realloc constraint grows it to the v2 footprint.
    await program.methods
      .migrate()
      .accounts({ authority: authority.publicKey, counter: legacy })
      .rpc();

    info = await program.provider.connection.getAccountInfo(legacy);
    assert.strictEqual(info!.data.length, V2_LEN, "account grown to v2 size");
    assert.strictEqual(
      info!.data.readBigUInt64LE(8).toString(),
      "42",
      "v1 data preserved"
    );

    const extra = await program.methods
      .readExtra()
      .accounts({ counter: legacy })
      .view();
    assert.strictEqual(extra.toString(), "84", "v2 field populated by migrate");
  });
});
