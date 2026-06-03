import path from "path";
import fs from "fs/promises";

import { BenchData, StackMemory, spawn } from "../scripts/utils";

const IDL = require("../target/idl/bench.json");

describe("Stack memory", () => {
  const stackMemory: StackMemory = {};

  const STACK_CONTENT = [
    "",
    `let stack_limit: [u16; 2048] = [1; 2048];`,
    `msg!("{}", stack_limit[2047]);`,
    "",
  ].join("\n\t\t");

  it("Measure stack memory usage", async () => {
    const libPath = path.join("programs", IDL.metadata.name, "src", "lib.rs");
    const lib = await fs.readFile(libPath, "utf8");
    const indices = [...lib.matchAll(/fn\s[\w\d]+\(/g)]
      .map((match) => match.index)
      .filter(Boolean) as number[];

    let modifiedLib = lib;
    let cumulativeIndex = 0;

    for (const index of indices) {
      const curlyIndex = index + lib.slice(index).indexOf("{");
      const nextLineIndex =
        curlyIndex + lib.slice(curlyIndex).indexOf("\n") + cumulativeIndex;
      modifiedLib =
        modifiedLib.slice(0, nextLineIndex) +
        STACK_CONTENT +
        modifiedLib.slice(nextLineIndex);

      cumulativeIndex += STACK_CONTENT.length;
    }

    // Write the modified file
    await fs.writeFile(libPath, modifiedLib);

    // Expected error:
    // Error: Function _ZN5bench9__private8__global13account_info117h88e5c10f03de9fddE
    // Stack offset of 4424 exceeded max offset of 4096 by 328 bytes
    const buildResult = spawn("anchor", [
      "build",
      "--skip-lint",
      "--ignore-keys",
    ]);
    const output = buildResult.output.toString();
    const matches = output.matchAll(
      /global[\d]+([\w\d]+?)17.*by\s(\d+)\sbytes/g
    );
    for (const match of matches) {
      const ixName = match[1];
      const stackUsage = match[2];
      stackMemory[ixName] = +stackUsage;
    }

    // Restore to the original file
    await fs.writeFile(libPath, lib);
  });

  after(async () => {
    const bench = await BenchData.open();
    await bench.update({ stackMemory });
  });
});
