import * as anchor from "@anchor-lang/core";

import { OverflowChecks } from "../../target/types/overflow_checks";

describe("overflow-checks", () => {
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .OverflowChecks as anchor.Program<OverflowChecks>;

  const accountKp = anchor.web3.Keypair.generate();
  const testOverflow = async (method: keyof typeof program["methods"]) => {
    try {
      await program.methods[method]()
        .accounts({
          account: accountKp.publicKey,
        })
        .rpc();
    } catch (e) {
      if (e.logs.some((log) => log.includes("with overflow"))) return;
      throw e;
    }

    throw new Error("Did not panic on overflow");
  };

  before(async () => {
    await program.methods
      .initialize()
      .accounts({
        account: accountKp.publicKey,
        payer: program.provider.publicKey,
      })
      .signers([accountKp])
      .rpc();
  });

  it("Panics on overflow add", async () => {
    await testOverflow("testOverflowAdd");
  });

  it("Panics on overflow sub", async () => {
    await testOverflow("testOverflowSub");
  });

  it("Panics on overflow mul", async () => {
    await testOverflow("testOverflowMul");
  });

  it("Fails to build when `overflow-checks` is not explicitly specified", async () => {
    const fs = await import("fs/promises");
    const cargoToml = await fs.readFile("Cargo.toml", { encoding: "utf8" });
    const CHECK = "overflow-checks = true";
    const COMMENTED_CHECK = "#" + CHECK;
    await fs.writeFile("Cargo.toml", cargoToml.replace(CHECK, COMMENTED_CHECK));

    const { spawnSync } = await import("child_process");
    const { stderr, status } = spawnSync("anchor", ["build"]);

    try {
      if (status === 0) {
        throw new Error(
          "Did not fail even though `overflow-checks` is not specified"
        );
      }

      if (!stderr.includes("Error: `overflow-checks` is not enabled")) {
        throw new Error(`Did not throw the correct overflow error:\n${stderr}`);
      }
    } catch (e) {
      throw e;
    } finally {
      await fs.writeFile(
        "Cargo.toml",
        cargoToml.replace(COMMENTED_CHECK, CHECK)
      );
    }
  });
});
