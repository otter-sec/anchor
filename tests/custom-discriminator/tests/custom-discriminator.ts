import * as anchor from "@anchor-lang/core";
import assert from "assert";

import type { CustomDiscriminator } from "../target/types/custom_discriminator";

describe("custom-discriminator", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program: anchor.Program<CustomDiscriminator> =
    anchor.workspace.customDiscriminator;

  describe("Instructions", () => {
    const testCommon = async (ixName: keyof typeof program["methods"]) => {
      const tx = await program.methods[ixName]().transaction();

      // Verify discriminator
      const ix = program.idl.instructions.find((ix) => ix.name === ixName)!;
      assert(ix.discriminator.length < 8);
      const data = tx.instructions[0].data;
      assert(data.equals(Buffer.from(ix.discriminator)));

      // Verify tx runs
      await program.provider.sendAndConfirm!(tx, [], {
        maxRetries: 3,
        skipPreflight: true,
      });
    };

    it("Integer", () => testCommon("int"));
    it("Array", () => testCommon("array"));
    it("Byte string", () => testCommon("byteStr"));
    it("Constant", () => testCommon("constant"));
    it("Const Fn", () => testCommon("constFn"));
  });

  describe("Accounts", () => {
    it("Works", async () => {
      // Verify discriminator
      const acc = program.idl.accounts.find((acc) => acc.name === "myAccount")!;
      assert(acc.discriminator.length < 8);

      // Verify regular `init` ix works
      const field = 5;
      const { pubkeys, signature } = await program.methods
        .account(field)
        .rpcAndKeys();
      await program.provider.connection.confirmTransaction(
        signature,
        "confirmed"
      );
      const myAccount = await program.account.myAccount.fetch(
        pubkeys.myAccount as anchor.web3.PublicKey
      );
      assert.strictEqual(myAccount.field, field);
    });
  });

  describe("Events", () => {
    it("Works", async () => {
      // Verify discriminator
      const event = program.idl.events.find((acc) => acc.name === "myEvent")!;
      assert(event.discriminator.length < 8);

      // Verify regular event works
      await new Promise<void>((res) => {
        const field = 5;
        const id = program.addEventListener("myEvent", (ev) => {
          assert.strictEqual(ev.field, field);
          program.removeEventListener(id);
          res();
        });
        program.methods.event(field).rpc();
      });
    });
  });
});
