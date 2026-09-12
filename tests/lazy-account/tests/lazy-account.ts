import * as anchor from "@anchor-lang/core";
import assert from "assert";

import type { LazyAccount } from "../target/types/lazy_account";

describe("lazy-account", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const program: anchor.Program<LazyAccount> = anchor.workspace.lazyAccount;

  it("Can init", async () => {
    const { pubkeys, signature } = await program.methods.init().rpcAndKeys();
    await program.provider.connection.confirmTransaction(
      signature,
      "confirmed"
    );
    const myAccount = await program.account.myAccount.fetch(pubkeys.myAccount);
    assert(myAccount.authority.equals(program.provider.publicKey!));
  });

  it("Can read", async () => {
    await program.methods.read().rpc();
  });

  it("Can write", async () => {
    const newAuthority = anchor.web3.PublicKey.default;
    const { pubkeys, signature } = await program.methods
      .write(newAuthority)
      .rpcAndKeys();
    await program.provider.connection.confirmTransaction(
      signature,
      "confirmed"
    );
    const myAccount = await program.account.myAccount.fetch(pubkeys.myAccount);
    assert(myAccount.authority.equals(newAuthority));
  });
});
