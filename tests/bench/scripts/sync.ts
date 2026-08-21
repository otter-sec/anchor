/**
 * Sync all saved data by re-running the tests for each version.
 *
 * This script should be used when the bench program or its tests has changed
 * and all data needs to be updated.
 */

import * as fs from "fs/promises";
import path from "path";

import {
  BENCHMARK_IDL_ENV,
  BENCHMARK_VERSION_ENV,
  BenchData,
  LockFile,
  PlatformToolsVersion,
  Toml,
  Version,
  VersionManager,
  spawn,
  usesLegacyIdlGeneration,
} from "./utils";

const CARGO_LOCK_PATH = "Cargo.lock";
const PROGRAM_MANIFEST_PATH = path.join("programs", "bench", "Cargo.toml");
const ANCHOR_TOML_PATH = path.join(__dirname, "..", "Anchor.toml");
const IDL_PATH = path.join("target", "idl", "bench.json");
const CURRENT_IDL_PATH = path.join("target", "bench-current-idl.json");
(async () => {
  const bench = await BenchData.open();

  const cargoToml = await Toml.open(
    path.join("..", "programs", "bench", "Cargo.toml")
  );
  const originalAnchorToml = await fs.readFile(ANCHOR_TOML_PATH, "utf8");

  const versions = bench.getVersions();
  const unreleased = bench.get("unreleased");
  VersionManager.setSolanaVersion(unreleased.solanaVersion);

  const buildEnv = {
    ...process.env,
    RUSTC_BOOTSTRAP: "1",
    RUSTFLAGS: "-Z emit-stack-sizes",
  };

  const setProjectVersion = async (version: Version) => {
    // Reopen the benchmark data because previous iterations update it in a
    // separate test process.
    const currentBench = await BenchData.open();
    const solanaVersion = currentBench.get(version).solanaVersion;
    const platformToolsResult = spawn(
      "avm",
      [
        "platform-tools",
        "resolve",
        "--solana-version",
        solanaVersion,
        "--output",
        "version",
      ],
      {
        throwOnError: {
          msg: `Failed to resolve platform-tools for Solana ${solanaVersion}.`,
        },
      }
    );
    const platformToolsOutput = platformToolsResult.stdout.toString().trim();
    if (!/^v\d+\.\d+$/.test(platformToolsOutput)) {
      throw new Error(
        `AVM returned an invalid platform-tools version: ${platformToolsOutput}.`
      );
    }
    const platformToolsVersion = platformToolsOutput as PlatformToolsVersion;
    currentBench.setPlatformToolsVersion(version, platformToolsVersion);
    await currentBench.save();

    const isUnreleased = version === "unreleased";

    await LockFile.replace(version);
    VersionManager.setSolanaVersion(currentBench.get(version).solanaVersion);

    cargoToml.replaceValue("idl-build", () => {
      return usesLegacyIdlGeneration(version)
        ? "[]"
        : '["anchor-lang/idl-build", "anchor-spl/idl-build"]';
    });

    for (const dependency of ["lang", "spl"]) {
      cargoToml.replaceValue(`anchor-${dependency}`, () => {
        return isUnreleased
          ? `{ path = "../../../../${dependency}" }`
          : `"${version}"`;
      });
    }
    await cargoToml.save();

    await fs.writeFile(
      ANCHOR_TOML_PATH,
      isUnreleased
        ? originalAnchorToml
        : `${originalAnchorToml.trimEnd()}\n\n[toolchain]\nanchor_version = "${version}"\n`
    );

    if (!isUnreleased) {
      spawn("avm", ["install", version], {
        throwOnError: { msg: `Failed to install Anchor CLI ${version}.` },
      });
    }

    return platformToolsVersion;
  };

  try {
    // The current TypeScript client needs the current IDL format, including
    // when a historical CLI is responsible for starting the validator.
    await fs.rm(IDL_PATH, { force: true });
    const buildResult = spawn("anchor", ["build", "--skip-lint"]);
    if (buildResult.status !== 0) {
      throw new Error("Failed to build the current benchmark program.");
    }
    await fs.copyFile(IDL_PATH, CURRENT_IDL_PATH);

    for (const version of versions) {
      console.log(`Updating '${version}'...`);

      const expectedPlatformToolsVersion = await setProjectVersion(version);

      const cargoBuildSbfVersionResult = spawn(
        "cargo-build-sbf",
        ["--version"],
        {
          throwOnError: { msg: "Failed to read the platform-tools version." },
        }
      );
      const actualPlatformToolsVersion =
        /(?:sbf|platform)-tools (v\d+\.\d+)/.exec(
          cargoBuildSbfVersionResult.stdout.toString()
        )?.[1];
      if (actualPlatformToolsVersion !== expectedPlatformToolsVersion) {
        throw new Error(
          `Expected platform-tools ${expectedPlatformToolsVersion}, found ${actualPlatformToolsVersion}.`
        );
      }

      // Resolve path dependencies in the cached lockfile before using the
      // version's Cargo. Keep the original lockfile format for old Cargo
      // versions that do not understand version 4.
      let lockFileVersion: string | undefined;
      try {
        const lockFile = await fs.readFile(CARGO_LOCK_PATH, "utf8");
        lockFileVersion = /^version = (\d+)$/m.exec(lockFile)?.[1];
        if (!lockFileVersion) {
          throw new Error("Failed to read lockfile version.");
        }
      } catch (err) {
        if (version !== "unreleased") throw err;
      }

      spawn(
        "cargo",
        [
          "metadata",
          "--format-version=1",
          "--features",
          "no-entrypoint",
          "--manifest-path",
          PROGRAM_MANIFEST_PATH,
        ],
        {
          maxBuffer: 16 * 1024 * 1024,
          throwOnError: { msg: "Failed to resolve benchmark dependencies." },
        }
      );
      if (lockFileVersion && lockFileVersion !== "4") {
        const resolvedLockFile = await fs.readFile(CARGO_LOCK_PATH, "utf8");
        await fs.writeFile(
          CARGO_LOCK_PATH,
          resolvedLockFile.replace(
            /^version = \d+$/m,
            `version = ${lockFileVersion}`
          )
        );
      }

      // Ensure the instrumented build replaces any artifact left by the
      // initial current-IDL build or the previous iteration.
      await fs.rm(path.join("target", "deploy", "bench.so"), { force: true });
      const buildResult = spawn(
        "cargo-build-sbf",
        ["--manifest-path", PROGRAM_MANIFEST_PATH, "--", "--locked"],
        { env: buildEnv }
      );
      if (buildResult.status !== 0) {
        console.error("Please fix the error and re-run this command.");
        process.exitCode = 1;
        return;
      }

      const result = spawn("anchor", ["test", "--skip-lint", "--skip-build"], {
        env: {
          ...buildEnv,
          [BENCHMARK_IDL_ENV]: path.resolve(CURRENT_IDL_PATH),
          [BENCHMARK_VERSION_ENV]: version,
        },
      });

      if (result.status !== 0) {
        console.error("Please fix the error and re-run this command.");
        process.exitCode = 1;
        return;
      }
    }

    spawn("anchor", ["run", "sync-markdown"]);
  } finally {
    await fs.rm(CURRENT_IDL_PATH, { force: true });
    await setProjectVersion("unreleased");
  }
})();
