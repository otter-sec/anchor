import * as fs from "fs";
import * as path from "path";
import * as toml from "toml";
import camelcase from "camelcase";
import { execSync } from "child_process";
import { Program } from "./program/index.js";
import { isBrowser } from "./utils/common.js";
import { Idl } from "./idl.js";

let cargoTargetDirectoryCache: string | undefined;

/**
 * Resolves the IDL file name in `idlDirPath` matching `programName`
 * (already camelCased). Throws a descriptive error if the directory
 * is missing, empty, or no match is found.
 */
export function resolveIdlFileName(
  idlDirPath: string,
  programName: string
): string {
  let dirEntries: string[];
  try {
    dirEntries = fs.readdirSync(idlDirPath);
  } catch (err: any) {
    if (err.code === "ENOENT") {
      throw new Error(
        `IDL directory not found at \`${idlDirPath}\`. Did you run \`anchor build\`?`
      );
    }
    throw err;
  }

  const jsonFiles = dirEntries.filter(
    (name: string) =>
      path.extname(name) === ".json" &&
      fs.statSync(path.join(idlDirPath, name)).isFile()
  );

  const fileName = jsonFiles.find(
    (name: string) => camelcase(path.parse(name).name) === programName
  );

  if (!fileName) {
    if (jsonFiles.length === 0) {
      throw new Error(
        `No IDL files found in \`${idlDirPath}\`. Did you run \`anchor build\`?`
      );
    }
    const available = jsonFiles
      .map((n: string) => path.parse(n).name)
      .sort()
      .join(", ");
    throw new Error(
      `Failed to find IDL for program \`${programName}\`.\n` +
        `Available programs in \`${idlDirPath}\`: ${available}\n` +
        `Ensure the following all use the same snake_case name:\n` +
        `  - \`[lib].name\` in Cargo.toml\n` +
        `  - \`#[program]\` module name in your Rust program\n` +
        `  - Program key in Anchor.toml under \`[programs.<cluster>]\``
    );
  }

  return fileName;
}

function getCargoTargetDirectory(): string {
  if (cargoTargetDirectoryCache !== undefined) {
    return cargoTargetDirectoryCache;
  }
  let metadata: { target_directory: string };
  try {
    const output = execSync("cargo metadata --no-deps --format-version=1", {
      encoding: "utf8",
    });
    metadata = JSON.parse(output);
  } catch (err) {
    throw new Error(
      `Failed to run 'cargo metadata'. Ensure Rust and Cargo are installed and the project is valid.\nOriginal error: ${
        err instanceof Error ? err.message : err
      }`
    );
  }
  cargoTargetDirectoryCache = metadata.target_directory;
  return cargoTargetDirectoryCache;
}

/**
 * The `workspace` namespace provides a convenience API to automatically
 * search for and deserialize [[Program]] objects defined by compiled IDLs
 * in an Anchor workspace.
 *
 * This API is for Node only.
 */
const workspace = new Proxy(
  {},
  {
    get(workspaceCache: { [key: string]: Program }, programName: string) {
      if (isBrowser) {
        throw new Error("Workspaces aren't available in the browser");
      }

      // Converting `programName` to camelCase enables the ability to use any
      // of the following to access the workspace program:
      // `workspace.myProgram`, `workspace.MyProgram`, `workspace["my-program"]`...
      programName = camelcase(programName);

      // Return early if the program is in cache
      if (workspaceCache[programName]) return workspaceCache[programName];

      // Override the workspace programs if the user put them in the config.
      const anchorToml = toml.parse(fs.readFileSync("Anchor.toml", "utf8"));
      const clusterId = anchorToml.provider.cluster;
      const programs = anchorToml.programs?.[clusterId];
      let programEntry;
      if (programs) {
        programEntry = Object.entries(programs).find(
          ([key]) => camelcase(key) === programName
        )?.[1];
      }

      let idlPath: string;
      let programId;
      if (typeof programEntry === "object" && programEntry.idl) {
        idlPath = programEntry.idl;
        programId = programEntry.address;
      } else {
        // Assuming the IDL file's name to be the snake_case name of the
        // `programName` with `.json` extension results in problems when
        // numbers are involved due to the nature of case conversion from
        // camelCase to snake_case being lossy.
        //
        // To avoid the above problem with numbers, read the `idl` directory and
        // compare the camelCased  version of both file names and `programName`.
        const idlDirPath = path.join(getCargoTargetDirectory(), "idl");
        const fileName = resolveIdlFileName(idlDirPath, programName);
        idlPath = path.join(idlDirPath, fileName);
      }

      if (!fs.existsSync(idlPath)) {
        throw new Error(
          `${idlPath} doesn't exist. Did you run \`anchor build\`?`
        );
      }

      const idl: Idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
      if (programId) {
        idl.address = programId;
      }
      workspaceCache[programName] = new Program(idl);

      return workspaceCache[programName];
    },
  }
);

export default workspace;
