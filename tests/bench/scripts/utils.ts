import * as fs from "fs/promises";
import path from "path";
import { execSync, spawnSync } from "child_process";

/** Version that is used in bench data file */
export type Version = "unreleased" | (`${number}.${number}.${number}` & {});

/** Persistent benchmark data(mapping of `Version -> Data`) */
type Bench = {
  [key: string]: {
    /**
     * Storing Solana version used in the release to:
     * - Be able to build older versions
     * - Adjust for the changes in platform-tools
     */
    solanaVersion: Version;
    /** Benchmark results for a version */
    result: BenchResult;
  };
};

/** Benchmark result per version */
export type BenchResult = {
  /** Benchmark result for program binary size */
  binarySize: BinarySize;
  /** Benchmark result for compute units consumed */
  computeUnits: ComputeUnits;
  /** Benchmark result for stack memory usage */
  stackMemory: StackMemory;
};

/** `program name -> binary size` */
export type BinarySize = { [programName: string]: number };

/** `instruction name -> compute units consumed` */
export type ComputeUnits = { [ixName: string]: number };

/** `instruction name -> stack memory used` */
export type StackMemory = { [ixName: string]: number };

/**
 * How much of a percentage difference between the current and the previous data
 * should be significant. Any difference above this number should be noted in
 * the benchmark file.
 */
export const THRESHOLD_PERCENTAGE = 1;

/** Path to the benchmark Markdown files */
export const BENCH_DIR_PATH = path.join("..", "..", "bench");

/** Command line argument for Anchor version */
export const ANCHOR_VERSION_ARG = "--anchor-version";

/** Utility class to handle benchmark data related operations */
export class BenchData {
  /** Benchmark data filepath */
  static #PATH = "bench.json";

  /** Benchmark data */
  #data: Bench;

  constructor(data: Bench) {
    this.#data = data;
  }

  /** Open the benchmark data file. */
  static async open() {
    let bench: Bench;
    try {
      const benchFile = await fs.readFile(BenchData.#PATH, {
        encoding: "utf8",
      });
      bench = JSON.parse(benchFile);
    } catch {
      bench = {};
    }

    return new BenchData(bench);
  }

  /** Save the benchmark data file. */
  async save() {
    await fs.writeFile(BenchData.#PATH, JSON.stringify(this.#data, null, 2));
  }

  /** Get the stored results based on version. */
  get(version: Version) {
    return this.#data[version];
  }

  /** Get all versions. */
  getVersions() {
    return Object.keys(this.#data) as Version[];
  }

  /** Compare benchmark changes. */
  compare<K extends keyof BenchResult>({
    newResult,
    oldResult,
    changeCb,
    noChangeCb,
    treshold = 0,
  }: {
    /** New bench result */
    newResult: BenchResult[K];
    /** Old bench result */
    oldResult: BenchResult[K];
    /** Callback to run when there is a change(considering `threshold`) */
    changeCb: (args: {
      name: string;
      newValue: number | null;
      oldValue: number | null;
    }) => void;
    /** Callback to run when there is no change(considering `threshold`) */
    noChangeCb?: (args: { name: string; value: number }) => void;
    /** Change threshold percentage(maximum allowed difference between results) */
    treshold?: number;
  }) {
    let needsUpdate = false;
    const executeChangeCb = (...args: Parameters<typeof changeCb>) => {
      changeCb(...args);
      needsUpdate = true;
    };

    const compare = (
      compareFrom: BenchResult[K],
      compareTo: BenchResult[K],
      cb: (name: string, value: number) => void
    ) => {
      for (const name in compareFrom) {
        if (compareTo[name] === undefined) {
          cb(name, compareFrom[name]);
        }
      }
    };

    // New key
    compare(newResult, oldResult, (name, value) => {
      console.log(`New key '${name}'`);
      executeChangeCb({
        name,
        newValue: value,
        oldValue: null,
      });
    });

    // Deleted key
    compare(oldResult, newResult, (name, value) => {
      console.log(`Deleted key '${name}'`);
      executeChangeCb({
        name,
        newValue: null,
        oldValue: value,
      });
    });

    // Compare compute units changes
    for (const name in newResult) {
      const oldValue = oldResult[name];
      const newValue = newResult[name];

      const percentage = treshold / 100;
      const oldMaximumAllowedDelta = oldValue * percentage;
      const newMaximumAllowedDelta = newValue * percentage;

      const delta = newValue - oldValue;
      const absDelta = Math.abs(delta);

      if (
        absDelta > oldMaximumAllowedDelta ||
        absDelta > newMaximumAllowedDelta
      ) {
        // Throw in CI
        if (process.env.CI) {
          throw new Error(
            [
              `Key '${name}' has changed more than ${treshold}% but is not saved.`,
              "Run `anchor test --skip-lint` in tests/bench and commit the changes.",
            ].join(" ")
          );
        }

        console.log(`'${name}' (${oldValue} -> ${newValue})`);

        executeChangeCb({
          name,
          newValue,
          oldValue,
        });
      } else {
        noChangeCb?.({ name, value: newValue });
      }
    }

    return { needsUpdate };
  }

  /** Compare and update benchmark changes. */
  async update(result: Partial<BenchResult>) {
    const resultType = Object.keys(result)[0] as keyof typeof result;
    const newResult = result[resultType]!;

    // Compare and update benchmark changes
    const version = getVersionFromArgs();
    const oldResult = this.get(version).result[resultType];
    const { needsUpdate } = this.compare({
      newResult,
      oldResult,
      changeCb: ({ name, newValue }) => {
        if (newValue === null) delete oldResult[name];
        else oldResult[name] = newValue;
      },
      treshold: THRESHOLD_PERCENTAGE,
    });

    if (needsUpdate) {
      console.log("Updating benchmark files...");

      // Save bench data file
      // (needs to happen before running the `sync-markdown` script)
      await this.save();

      // Only update markdown files on `unreleased` version
      if (version === "unreleased") {
        spawn("anchor", ["run", "sync-markdown"]);
      }
    }
  }

  /** Bump benchmark data version to the given version. */
  bumpVersion(newVersion: string) {
    if (this.#data[newVersion]) {
      throw new Error(`Version '${newVersion}' already exists!`);
    }

    const versions = this.getVersions();
    const unreleasedVersion = versions[versions.length - 1];

    // Add the new version
    this.#data[newVersion] = this.get(unreleasedVersion);

    // Delete the unreleased version
    delete this.#data[unreleasedVersion];

    // Add the new unreleased version
    this.#data[unreleasedVersion] = this.#data[newVersion];
  }

  /**
   * Loop through all of the markdown files and run the given callback before
   * saving the file.
   */
  static async forEachMarkdown(
    cb: (markdown: Markdown, fileName: string) => void
  ) {
    const fileNames = await fs.readdir(BENCH_DIR_PATH);
    const markdownFileNames = fileNames.filter((n) => n.endsWith(".md"));

    for (const fileName of markdownFileNames) {
      const markdown = await Markdown.open(path.join(BENCH_DIR_PATH, fileName));
      cb(markdown, fileName);
      await markdown.save();
    }

    // Format
    spawn("yarn", [
      "run",
      "prettier",
      "--write",
      path.join(BENCH_DIR_PATH, "*.md"),
    ]);
  }
}

/** Utility class to handle markdown related operations */
export class Markdown {
  /** Unreleased version string */
  static #UNRELEASED_VERSION = "[Unreleased]";

  /** Markdown filepath */
  #path: string;

  /** Markdown text */
  #text: string;

  constructor(path: string, text: string) {
    this.#path = path;
    this.#text = text;
  }

  /** Open the markdown file. */
  static async open(path: string) {
    const text = await fs.readFile(path, { encoding: "utf8" });
    return new Markdown(path, text);
  }

  /** Create a markdown table. */
  static createTable(...args: string[]) {
    return new MarkdownTable([args]);
  }

  /** Save the markdown file. */
  async save() {
    await fs.writeFile(this.#path, this.#text);
  }

  /** Change the version's content with the given `solanaVersion` and `table`. */
  updateVersion(params: {
    version: Version;
    solanaVersion: string;
    table: MarkdownTable;
  }) {
    const md = this.#text;

    const title = `[${params.version}]`;
    let titleStartIndex = md.indexOf(title);
    if (titleStartIndex === -1) {
      titleStartIndex = md.indexOf(Markdown.#UNRELEASED_VERSION);
    }

    const titleContentStartIndex = titleStartIndex + title.length + 1;

    const tableStartIndex =
      titleStartIndex + md.slice(titleStartIndex).indexOf("|");
    const tableRowStartIndex =
      tableStartIndex + md.slice(tableStartIndex).indexOf("\n");
    const tableEndIndex =
      tableStartIndex + md.slice(tableStartIndex).indexOf("\n\n");

    this.#text =
      md.slice(0, titleContentStartIndex) +
      `Solana version: ${params.solanaVersion}\n\n` +
      md.slice(tableStartIndex, tableRowStartIndex - 1) +
      params.table.toString() +
      md.slice(tableEndIndex + 1);
  }

  /** Bump the version to the given version. */
  bumpVersion(newVersion: string) {
    newVersion = `[${newVersion}]`;
    if (this.#text.includes(newVersion)) {
      throw new Error(`Version '${newVersion}' already exists!`);
    }

    const startIndex = this.#text.indexOf(`## ${Markdown.#UNRELEASED_VERSION}`);
    const endIndex =
      startIndex + this.#text.slice(startIndex).indexOf("\n---") + 4;
    let unreleasedSection = this.#text.slice(startIndex, endIndex);

    // Update unreleased version to `newVersion`
    const newSection = unreleasedSection.replace(
      Markdown.#UNRELEASED_VERSION,
      newVersion
    );

    // Reset unreleased version changes
    unreleasedSection = unreleasedSection
      .split("\n")
      .map((line, i) => {
        // First 4 lines don't change
        if ([0, 1, 2, 3].includes(i)) return line;

        const regex = /\|.*\|.*\|(.*)\|/;
        const result = regex.exec(line);

        const changeStr = result?.[1];
        if (!changeStr) {
          if (line.startsWith("#")) return line;
          else if (line.startsWith("---")) return line + "\n";
          else return "";
        }

        return line.replace(changeStr, "-");
      })
      .join("\n");

    // Update the text
    this.#text =
      this.#text.slice(0, startIndex) +
      unreleasedSection +
      newSection +
      this.#text.slice(endIndex);
  }
}

/** Utility class to handle markdown table related operations */
class MarkdownTable {
  /** Markdown rows stored as array of arrays */
  #rows: string[][];

  constructor(rows: string[][]) {
    this.#rows = rows;
    this.insert("-", "-", "-");
  }

  /** Insert a new row to the markdown table. */
  insert(...args: string[]) {
    this.#rows.push(args);
  }

  /** Convert the stored rows to a markdown table. */
  toString() {
    return this.#rows.reduce(
      (acc, row) =>
        acc + row.reduce((acc, cur) => `${acc} ${cur} |`, "|") + "\n",
      ""
    );
  }
}

/** Utility class to handle TOML related operations */
export class Toml {
  /** TOML filepath */
  #path: string;

  /** TOML text */
  #text: string;

  constructor(path: string, text: string) {
    this.#path = path;
    this.#text = text;
  }

  /** Open the TOML file. */
  static async open(tomlPath: string) {
    tomlPath = path.join(__dirname, tomlPath);
    const text = await fs.readFile(tomlPath, {
      encoding: "utf8",
    });
    return new Toml(tomlPath, text);
  }

  /** Save the TOML file. */
  async save() {
    await fs.writeFile(this.#path, this.#text);
  }

  /** Replace the value for the given key. */
  replaceValue(
    key: string,
    cb: (previous: string) => string,
    opts?: { insideQuotes: boolean }
  ) {
    this.#text = this.#text.replace(
      new RegExp(`${key}\\s*=\\s*${opts?.insideQuotes ? `"(.*)"` : "(.*)"}`),
      (line, value) => line.replace(value, cb(value))
    );
  }
}

/** Utility class to handle Cargo.lock file related operations */
export class LockFile {
  /** Cargo lock file name */
  static #CARGO_LOCK = "Cargo.lock";

  /** Replace the Cargo.lock with the given version's cached lock file. */
  static async replace(version: Version) {
    // Remove Cargo.lock
    try {
      await fs.rm(this.#CARGO_LOCK);
    } catch {}

    // `unreleased` version shouldn't have a cached lock file
    if (version !== "unreleased") {
      const lockFile = await fs.readFile(this.#getLockPath(version));
      await fs.writeFile(this.#CARGO_LOCK, lockFile);
    }
  }

  /** Cache the current Cargo.lock in `./locks`. */
  static async cache(version: Version) {
    try {
      await fs.rename(this.#CARGO_LOCK, this.#getLockPath(version));
    } catch {
      // Lock file doesn't exist
      // Run the tests to create the lock file
      const result = runAnchorTest();

      // Check failure
      if (result.status !== 0) {
        throw new Error(`Failed to create ${this.#CARGO_LOCK}`);
      }

      await this.cache(version);
    }
  }

  /** Get the lock file path from the given version. */
  static #getLockPath(version: Version) {
    return path.join("locks", `${version}.lock`);
  }
}

/** Utility class to manage versions */
export class VersionManager {
  /** Set the active Solana version with `solana-install init` command. */
  static setSolanaVersion(version: Version) {
    const activeVersion = this.#getSolanaVersion();
    if (activeVersion === version) return;

    // `solana-install` is renamed to `agave-install` in Solana v2
    // https://github.com/anza-xyz/agave/wiki/Agave-Transition
    const cmdName = activeVersion.startsWith("1")
      ? "solana-install"
      : "agave-install";
    spawn(cmdName, ["init", version], {
      logOutput: true,
      throwOnError: { msg: `Failed to set Solana version to ${version}` },
    });
  }

  /** Get the active Solana version. */
  static #getSolanaVersion() {
    // `solana-cli 1.14.16 (src:0fb2ffda; feat:3488713414)\n`
    const result = execSync("solana --version");
    const output = Buffer.from(result.buffer).toString();
    const solanaVersion = /(\d\.\d{1,3}\.\d{1,3})/.exec(output)![1].trim();
    return solanaVersion as Version;
  }
}

/**
 * Get Anchor version from the passed arguments.
 *
 * Defaults to `unreleased`.
 */
export const getVersionFromArgs = () => {
  const args = process.argv;
  const anchorVersionArgIndex = args.indexOf(ANCHOR_VERSION_ARG);
  return anchorVersionArgIndex === -1
    ? "unreleased"
    : (args[anchorVersionArgIndex + 1] as Version);
};

/** Spawn a blocking process. */
export const spawn = (
  cmd: string,
  args: string[],
  opts?: { logOutput?: boolean; throwOnError?: { msg: string } }
) => {
  const result = spawnSync(cmd, args);
  const success = result.status === 0;
  if (opts?.logOutput || !success) {
    console.log(
      `Output of \`${cmd} ${args.join(" ")}\`:`,
      result.output.toString()
    );
  }

  if (opts?.throwOnError && !success) {
    throw new Error(opts.throwOnError.msg);
  }

  return result;
};

/** Run `anchor test` command. */
export const runAnchorTest = () => spawn("anchor", ["test", "--skip-lint"]);

/** Format number with `en-US` locale. */
export const formatNumber = (number: number) => number.toLocaleString("en-US");
