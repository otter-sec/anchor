/**
 * Sync all saved data by re-running the tests for each version.
 *
 * This script should be used when the bench program or its tests has changed
 * and all data needs to be updated.
 */

import * as fs from "fs/promises";
import os from "os";
import path from "path";

import {
  BENCHMARK_VERSION_ENV,
  BenchData,
  LockFile,
  PlatformToolsVersion,
  Toml,
  Version,
  spawn,
} from "./utils";

const CARGO_LOCK_PATH = "Cargo.lock";
const PROGRAM_MANIFEST_PATH = path.join("programs", "bench", "Cargo.toml");
const ANCHOR_TOML_PATH = path.join(__dirname, "..", "Anchor.toml");
const IDL_PATH = path.join("target", "idl", "bench.json");
(async () => {
  const bench = await BenchData.open();

  const cargoToml = await Toml.open(
    path.join("..", "programs", "bench", "Cargo.toml")
  );
  const anchorToml = await Toml.open(path.join("..", "Anchor.toml"));
  const originalAnchorToml = await fs.readFile(ANCHOR_TOML_PATH, "utf8");

  const tags = spawn(
    "git",
    [
      "ls-remote",
      "--tags",
      "--refs",
      "https://github.com/otter-sec/anchor.git",
    ],
    { throwOnError: { msg: "Failed to list published benchmark versions." } }
  ).stdout.toString();
  const taggedVersions = new Set(
    [...tags.matchAll(/refs\/tags\/v([^\s]+)/g)].map((match) => match[1])
  );
  const versions = bench.getVersions().filter((version) => {
    if (bench.get(version).disabled) return false;
    if (version === "unreleased" || taggedVersions.has(version)) return true;
    console.log(`Skipping untagged release snapshot '${version}'.`);
    return false;
  });
  const buildEnv: NodeJS.ProcessEnv = {
    ...process.env,
    RUSTC_BOOTSTRAP: "1",
    CARGO_TARGET_SBF_SOLANA_SOLANA_RUSTFLAGS: "-Z emit-stack-sizes",
    CARGO_TARGET_SBPF_SOLANA_SOLANA_RUSTFLAGS: "-Z emit-stack-sizes",
    CARGO_TARGET_SBPFV1_SOLANA_SOLANA_RUSTFLAGS: "-Z emit-stack-sizes",
    CARGO_TARGET_SBPFV2_SOLANA_SOLANA_RUSTFLAGS: "-Z emit-stack-sizes",
    CARGO_TARGET_SBPFV3_SOLANA_SOLANA_RUSTFLAGS: "-Z emit-stack-sizes",
  };
  // Sync intentionally records changed measurements, unlike the CI test job.
  delete buildEnv.CI;

  const setProjectVersion = async (version: Version) => {
    // Reopen the benchmark data because previous iterations update it in a
    // separate test process.
    const currentBench = await BenchData.open();
    const solanaVersion = currentBench.get(version).solanaVersion;
    let platformToolsVersion = currentBench.get(version).platformToolsVersion;
    // Historical measurements must retain their recorded compiler version.
    if (version === "unreleased") {
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
      if (!/^v\d+\.\d+(?:\.\d+)?$/.test(platformToolsOutput)) {
        throw new Error(
          `AVM returned an invalid platform-tools version: ${platformToolsOutput}.`
        );
      }
      platformToolsVersion = platformToolsOutput as PlatformToolsVersion;
      currentBench.setPlatformToolsVersion(version, platformToolsVersion);
      await currentBench.save();
    }

    const isUnreleased = version === "unreleased";

    await LockFile.replace(version);
    for (const dependency of ["lang", "spl"]) {
      cargoToml.replaceValue(`anchor-${dependency}`, () => {
        return isUnreleased
          ? `{ path = "../../../../${dependency}" }`
          : `"${version}"`;
      });
    }
    cargoToml.replaceValue("idl-build", () =>
      ["0.27.0", "0.28.0"].includes(version)
        ? "[]"
        : '["anchor-lang/idl-build", "anchor-spl/idl-build"]'
    );
    await cargoToml.save();

    anchorToml.replaceValue(
      "anchor_version",
      () => (isUnreleased ? "" : version),
      { insideQuotes: true }
    );
    anchorToml.replaceValue("solana_version", () => solanaVersion, {
      insideQuotes: true,
    });
    await anchorToml.save();

    if (!isUnreleased) {
      spawn("avm", ["install", version], {
        throwOnError: { msg: `Failed to install Anchor CLI ${version}.` },
      });
    }
    spawn("avm", ["solana", "install"], {
      throwOnError: { msg: `Failed to install Solana ${solanaVersion}.` },
    });

    // AVM keeps explicit platform-tools installs under its own home directory,
    // while older cargo-build-sbf releases look only in Solana's shared cache.
    // Link the resolved release into that legacy location before building.
    spawn("avm", ["platform-tools", "install", platformToolsVersion], {
      throwOnError: {
        msg: `Failed to install platform-tools ${platformToolsVersion}.`,
      },
    });
    const avmHome = process.env.AVM_HOME ?? path.join(os.homedir(), ".avm");
    const platformToolsSource = path.join(
      avmHome,
      "platform-tools",
      platformToolsVersion
    );
    const platformToolsDestination = path.join(
      os.homedir(),
      ".cache",
      "solana",
      platformToolsVersion,
      "platform-tools"
    );
    try {
      await fs.lstat(platformToolsDestination);
    } catch {
      await fs.mkdir(path.dirname(platformToolsDestination), {
        recursive: true,
      });
      await fs.symlink(
        platformToolsSource,
        platformToolsDestination,
        process.platform === "win32" ? "junction" : "dir"
      );
    }
  };

  try {
    await setProjectVersion("unreleased");
    // Build the IDL once with the current CLI. The TypeScript tests use this
    // format even when a historical CLI starts the validator.
    await fs.rm(IDL_PATH, { force: true });
    const buildResult = spawn("anchor", ["build", "--skip-lint"]);
    if (buildResult.status !== 0) {
      throw new Error("Failed to build the current benchmark program.");
    }

    for (const version of versions) {
      console.log(`Updating '${version}'...`);

      await setProjectVersion(version);
      const versionBuildEnv = {
        ...buildEnv,
        ANCHOR_BUILD_SBF_ARCH: bench.get(version).sbpfArch ?? "v3",
      };

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
      // initial current-IDL build or the previous iteration. Remove the
      // target-specific directories as well as target/deploy: Cargo otherwise
      // considers the old uninstrumented fingerprint fresh.
      await Promise.all([
        ...["sbf", "sbpf", "sbpfv1", "sbpfv2", "sbpfv3"].map((target) =>
          fs.rm(path.join("target", `${target}-solana-solana`), {
            force: true,
            recursive: true,
          })
        ),
        fs.rm(path.join("target", "deploy", "bench.so"), { force: true }),
      ]);
      const buildArgs = ["build", "--skip-lint", "--no-idl"];
      // Program ID checks were added in v1.0.0. Historical benchmark builds
      // use a generated keypair, so they must not require it to match the
      // fixed benchmark program ID.
      if (version === "unreleased" || version >= "1.0.0") {
        buildArgs.push("--ignore-keys");
      }
      const buildResult = spawn("anchor", buildArgs, {
        env: versionBuildEnv,
      });
      if (buildResult.status !== 0) {
        console.error("Please fix the error and re-run this command.");
        process.exitCode = 1;
        return;
      }

      const result = spawn("anchor", ["test", "--skip-lint", "--skip-build"], {
        env: {
          ...versionBuildEnv,
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
    try {
      await setProjectVersion("unreleased");
    } finally {
      await fs.writeFile(ANCHOR_TOML_PATH, originalAnchorToml);
    }
  }
})();
