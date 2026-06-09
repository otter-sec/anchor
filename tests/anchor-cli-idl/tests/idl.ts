import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { IdlCommandsOne } from "../target/types/idl_commands_one";
import { IdlCommandsTwo } from "../target/types/idl_commands_two";
import { assert } from "chai";
import { execSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { Keypair, PublicKey } from "@solana/web3.js";

const PROGRAM_METADATA_PROGRAM_ID = new PublicKey(
  "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S"
);
// Keep this in sync with the Rust historical fetch helper in `cli/src/fetch/pmp.rs`.
const IDL_METADATA_SEED = "idl";

describe("Test CLI IDL commands", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);

  const programOne = anchor.workspace.IdlCommandsOne as Program<IdlCommandsOne>;
  const programTwo = anchor.workspace.IdlCommandsTwo as Program<IdlCommandsTwo>;

  const fetchIdl = anchor.Program.fetchIdl;
  const historyDir = fs.mkdtempSync(
    path.join(os.tmpdir(), "anchor-cli-idl-history-")
  );

  const serializeIdl = (idl: unknown) => JSON.stringify(idl);
  const getNonCanonicalPmpIdlAddress = (authority: PublicKey) => {
    const paddedSeed = Buffer.alloc(16);
    Buffer.from(IDL_METADATA_SEED, "utf8").copy(paddedSeed);
    return PublicKey.findProgramAddressSync(
      [programOne.programId.toBuffer(), authority.toBuffer(), paddedSeed],
      PROGRAM_METADATA_PROGRAM_ID
    )[0];
  };
  const listFetchedIdlFiles = (dir: string) =>
    fs
      .readdirSync(dir)
      .filter((file) => /^idl_\d+(?:_(?:legacy|pmp))?\.json$/.test(file))
      .sort((a, b) => {
        const slotA = Number(a.match(/^idl_(\d+)/)?.[1] ?? 0);
        const slotB = Number(b.match(/^idl_(\d+)/)?.[1] ?? 0);
        return slotA - slotB;
      });
  const readFetchedIdls = (dir: string) =>
    listFetchedIdlFiles(dir).map((file) => ({
      file,
      slot: Number(file.match(/^idl_(\d+)/)?.[1] ?? 0),
      idl: JSON.parse(fs.readFileSync(path.join(dir, file), "utf8")),
    }));

  it("Can initialize IDL account", async () => {
    execSync(
      `anchor idl init --filepath target/idl/idl_commands_one.json --allow-localnet`,
      { stdio: "inherit" }
    );
  });

  it("Can fetch an IDL using the TypeScript client", async () => {
    const idl = await fetchIdl(programOne.programId, provider);
    assert.deepEqual(idl, programOne.rawIdl);
  });

  it("Can fetch an IDL via the CLI", async () => {
    const idl = execSync(`anchor idl fetch ${programOne.programId}`).toString();
    assert.deepEqual(JSON.parse(idl), programOne.rawIdl);
  });

  it("Can write a new IDL using the upgrade command", async () => {
    // Upgrade the IDL of program one to the IDL of program two to test upgrade
    execSync(
      `anchor idl upgrade --filepath target/idl/idl_commands_two.json --allow-localnet ${programOne.programId}`,
      { stdio: "inherit" }
    );
    const idl = await fetchIdl(programOne.programId, provider);
    assert.deepEqual(idl, programTwo.rawIdl);
  });

  it("Can fetch historical IDLs via the CLI", async () => {
    fs.rmSync(historyDir, { recursive: true, force: true });
    fs.mkdirSync(historyDir, { recursive: true });

    execSync(
      `anchor idl fetch-historical ${programOne.programId} --out-dir ${historyDir}`,
      { stdio: "inherit" }
    );

    const fetchedIdls = readFetchedIdls(historyDir);
    const fetchedFiles = fetchedIdls.map(({ file }) => file);

    assert.isAtLeast(fetchedIdls.length, 2);
    assert.deepInclude(
      fetchedIdls.map(({ idl }) => idl),
      programOne.rawIdl
    );
    assert.deepInclude(
      fetchedIdls.map(({ idl }) => idl),
      programTwo.rawIdl
    );

    const slotsByIdl = new Map(
      fetchedIdls.map(({ idl, slot }) => [serializeIdl(idl), slot])
    );
    assert.sameMembers(
      [...slotsByIdl.keys()].sort(),
      [serializeIdl(programOne.rawIdl), serializeIdl(programTwo.rawIdl)].sort()
    );
    assert.isTrue(
      fetchedFiles.every((file) => file.endsWith(".json")),
      "historical fetch should emit JSON files"
    );
  });

  it("Can fetch a historical IDL at a specific slot via the CLI", async () => {
    const slotDir = fs.mkdtempSync(
      path.join(os.tmpdir(), "anchor-cli-idl-slot-history-")
    );
    fs.rmSync(historyDir, { recursive: true, force: true });
    fs.mkdirSync(historyDir, { recursive: true });

    execSync(
      `anchor idl fetch-historical ${programOne.programId} --out-dir ${historyDir}`,
      { stdio: "inherit" }
    );

    // Rebuild the slot map from this test's own fetch output so the test does not
    // depend on state captured by a prior test.
    const localSlotsByIdl = new Map(
      readFetchedIdls(historyDir).map(({ idl, slot }) => [
        serializeIdl(idl),
        slot,
      ])
    );
    const oldestSlot = localSlotsByIdl.get(serializeIdl(programOne.rawIdl));
    assert.isDefined(
      oldestSlot,
      "program one should have a recorded historical slot"
    );

    execSync(
      `anchor idl fetch-historical ${
        programOne.programId
      } --out-dir ${slotDir} --slot ${oldestSlot!}`,
      { stdio: "inherit" }
    );

    const fetchedIdl = JSON.parse(
      fs.readFileSync(path.join(slotDir, `idl_${oldestSlot!}.json`), "utf8")
    );
    assert.deepEqual(fetchedIdl, programOne.rawIdl);
  });

  it("Can fetch non-canonical historical IDLs via the CLI", async () => {
    const nonCanonicalDir = fs.mkdtempSync(
      path.join(os.tmpdir(), "anchor-cli-idl-non-canonical-history-")
    );

    // PMP treats writes signed by the program upgrade authority as canonical even with
    // `--non-canonical`, so the non-canonical write must be signed by a different key.
    const nonCanonicalAuthority = Keypair.generate();
    const authorityKeypairPath = path.join(
      os.tmpdir(),
      `anchor-cli-idl-non-canonical-auth-${Date.now()}.json`
    );
    fs.writeFileSync(
      authorityKeypairPath,
      JSON.stringify(Array.from(nonCanonicalAuthority.secretKey))
    );

    execSync(
      `solana --keypair ${
        process.env.ANCHOR_WALLET ?? "./keypairs/deployer-keypair.json"
      } --url ${
        provider.connection.rpcEndpoint
      } transfer ${nonCanonicalAuthority.publicKey.toBase58()} 2 --allow-unfunded-recipient`,
      { stdio: "inherit" }
    );

    execSync(`anchor idl close ${programOne.programId}`, { stdio: "inherit" });

    try {
      execSync(
        `anchor idl init --non-canonical --filepath target/idl/idl_commands_one.json --allow-localnet --provider.wallet ${authorityKeypairPath} ${programOne.programId}`,
        { stdio: "inherit" }
      );

      const idlAccountAddress = getNonCanonicalPmpIdlAddress(
        nonCanonicalAuthority.publicKey
      );
      const signatures = await provider.connection.getSignaturesForAddress(
        idlAccountAddress,
        { limit: 20 },
        "confirmed"
      );
      assert.isAtLeast(signatures.filter((sig) => sig.err === null).length, 1);

      execSync(
        `anchor idl fetch-historical ${
          programOne.programId
        } --authority ${nonCanonicalAuthority.publicKey.toBase58()} --out-dir ${nonCanonicalDir}`,
        { stdio: "inherit" }
      );

      const fetchedIdls = readFetchedIdls(nonCanonicalDir).map(
        ({ idl }) => idl
      );
      assert.isAtLeast(fetchedIdls.length, 1);
      assert.deepInclude(fetchedIdls, programOne.rawIdl);
    } finally {
      // Each cleanup step runs independently and is best-effort: a failure here is reported
      // but does not mask the original test outcome or block subsequent cleanup steps.
      try {
        fs.rmSync(authorityKeypairPath, { force: true });
      } catch (cleanupErr) {
        console.error(
          `Failed to remove temporary authority keypair at ${authorityKeypairPath}:`,
          cleanupErr
        );
      }
      // Restore the canonical IDL account so the remaining CLI tests continue to exercise the
      // default fetch/close flows against the standard PMP metadata PDA.
      try {
        execSync(
          `anchor idl init --filepath target/idl/idl_commands_one.json --allow-localnet ${programOne.programId}`,
          { stdio: "inherit" }
        );
      } catch (restoreErr) {
        console.error(
          "Failed to restore canonical IDL account; subsequent tests may fail:",
          restoreErr
        );
      }
    }
  });

  // FIXME: Port this to the new create-buffer/write-buffer subcommands
  // it("Can write a new IDL using write-buffer and set-buffer", async () => {
  //   // "Upgrade" back to program one via write-buffer set-buffer
  //   let buffer = execSync(
  //     `anchor idl write-buffer --filepath target/idl/idl_commands_one.json ${programOne.programId}`
  //   ).toString();
  //   buffer = buffer.replace("Idl buffer created: ", "").trim();
  //   execSync(
  //     `anchor idl set-buffer --buffer ${buffer} ${programOne.programId}`,
  //     { stdio: "inherit" }
  //   );
  //   const idl = await anchor.Program.fetchIdl(programOne.programId, provider);
  //   assert.deepEqual(idl, programOne.rawIdl);
  // });

  it("Can close IDL account", async () => {
    execSync(`anchor idl close ${programOne.programId}`, { stdio: "inherit" });
    const idl = await fetchIdl(programOne.programId, provider);
    assert.isNull(idl);
  });

  it("Can initialize super massive IDL account", async () => {
    execSync(`anchor idl init --filepath testLargeIdl.json --allow-localnet`, {
      stdio: "inherit",
    });

    const idlActual = await fetchIdl(programOne.programId);
    const idlExpected = JSON.parse(
      fs.readFileSync("testLargeIdl.json", "utf8")
    );
    assert.deepEqual(idlActual, idlExpected);
  });

  it("Can initialize IDL account with relative path from subdirectory", async () => {
    execSync(`anchor idl close ${programOne.programId}`, { stdio: "inherit" });

    execSync(
      `cd target/idl && anchor idl init --filepath idl_commands_one.json --allow-localnet ${programOne.programId}`,
      { stdio: "inherit" }
    );

    const idl = await fetchIdl(programOne.programId, provider);
    assert.deepEqual(idl, programOne.rawIdl);
  });
});
