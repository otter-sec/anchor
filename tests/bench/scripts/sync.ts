/**
 * Sync all saved data by re-running the tests for each version.
 *
 * This script should be used when the bench program or its tests has changed
 * and all data needs to be updated.
 */

import path from "path";

import {
  ANCHOR_VERSION_ARG,
  BenchData,
  LockFile,
  Toml,
  VersionManager,
  runAnchorTest,
  spawn,
} from "./utils";

(async () => {
  const bench = await BenchData.open();

  const cargoToml = await Toml.open(
    path.join("..", "programs", "bench", "Cargo.toml")
  );
  const anchorToml = await Toml.open(path.join("..", "Anchor.toml"));

  for (const version of bench.getVersions()) {
    console.log(`Updating '${version}'...`);

    const isUnreleased = version === "unreleased";

    // Use the lock file from cache
    await LockFile.replace(version);

    // Set active solana version
    VersionManager.setSolanaVersion(bench.get(version).solanaVersion);

    // Update the anchor dependency versions
    for (const dependency of ["lang", "spl"]) {
      cargoToml.replaceValue(`anchor-${dependency}`, () => {
        return isUnreleased
          ? `{ path = "../../../../${dependency}" }`
          : `"${version}"`;
      });
    }

    // Save Cargo.toml
    await cargoToml.save();

    // Update `anchor test` command to pass version in Anchor.toml
    anchorToml.replaceValue(
      "test",
      (cmd) => {
        return cmd.includes(ANCHOR_VERSION_ARG)
          ? cmd.replace(
              new RegExp(`\\s*${ANCHOR_VERSION_ARG}\\s+(.+)`),
              (arg, ver) => (isUnreleased ? "" : arg.replace(ver, version))
            )
          : `${cmd} ${ANCHOR_VERSION_ARG} ${version}`;
      },
      { insideQuotes: true }
    );

    // Save Anchor.toml
    await anchorToml.save();

    // Run the command to update the current version's results
    const result = runAnchorTest();

    // Check failure
    if (result.status !== 0) {
      console.error("Please fix the error and re-run this command.");
      process.exitCode = 1;
      return;
    }
  }

  // Sync markdown files
  spawn("anchor", ["run", "sync-markdown"]);
})();
