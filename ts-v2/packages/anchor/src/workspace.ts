import * as toml from "toml";
import camelcase from "camelcase";
import { Program } from "./program/index.js";
import { isBrowser } from "./utils/common.js";
import { Idl } from "./idl.js";

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

      const fs = require("fs");
      const path = require("path");

      // Override the workspace programs if the user put them in the config.
      const anchorToml = toml.parse(fs.readFileSync("Anchor.toml"));
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
        const idlDirPath = path.join("target", "idl");
        const fileName = fs
          .readdirSync(idlDirPath)
          .find((name) => camelcase(path.parse(name).name) === programName);
        if (!fileName) {
          throw new Error(`Failed to find IDL of program \`${programName}\``);
        }

        idlPath = path.join(idlDirPath, fileName);
      }

      if (!fs.existsSync(idlPath)) {
        throw new Error(
          `${idlPath} doesn't exist. Did you run \`anchor build\`?`
        );
      }

      const idl: Idl = JSON.parse(fs.readFileSync(idlPath));
      if (programId) {
        idl.address = programId;
      }
      workspaceCache[programName] = new Program(idl);

      return workspaceCache[programName];
    },
  }
);

export default workspace;
