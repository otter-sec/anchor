import * as anchor from "@anchor-lang/core";
import { AnchorError, Program } from "@anchor-lang/core";
import splToken from "@solana/spl-token";
import { DeclareId } from "../target/types/declare_id";
import { assert } from "chai";

describe("declare_id", () => {
  anchor.setProvider(anchor.AnchorProvider.local());
  const program = anchor.workspace.DeclareId as Program<DeclareId>;

  it("throws error!", async () => {
    try {
      await program.methods.initialize().rpc();
      assert.ok(false);
    } catch (_err) {
      assert.isTrue(_err instanceof AnchorError);
      const err: AnchorError = _err;
      assert.strictEqual(err.error.errorCode.number, 4100);
    }
  });
});
