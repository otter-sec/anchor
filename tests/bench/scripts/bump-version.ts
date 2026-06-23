/**
 * Bump the version of all benchmark related files by changing the `Unreleased`
 * version to a new version and adding a new `Unreleased` version.
 */

import {
  ANCHOR_VERSION_ARG,
  BenchData,
  LockFile,
  getVersionFromArgs,
} from "./utils";

(async () => {
  const newVersion = getVersionFromArgs();

  if (newVersion === "unreleased") {
    console.error(
      `Usage: anchor run bump-version -- ${ANCHOR_VERSION_ARG} <VERSION>`
    );
    process.exitCode = 1;
    return;
  }

  // Ignore non-stable releases
  if (newVersion.includes("-")) return;

  // Cache lock file in ./locks
  await LockFile.cache(newVersion);

  // Bump bench data
  const bench = await BenchData.open();
  bench.bumpVersion(newVersion);
  await bench.save();

  // Bump markdown files
  await BenchData.forEachMarkdown((markdown) => {
    markdown.bumpVersion(newVersion);
  });
})();
