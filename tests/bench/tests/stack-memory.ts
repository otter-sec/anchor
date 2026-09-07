import path from "path";
import fs from "fs/promises";
import os from "os";

import {
  BenchData,
  StackMemory,
  getVersionFromArgs,
  spawn,
} from "../scripts/utils";

describe("Stack memory", () => {
  const stackMemory: StackMemory = {};

  const getInstructionAccountsStructs = async () => {
    const lib = await fs.readFile(
      path.join("programs", "bench", "src", "lib.rs"),
      "utf8"
    );
    const structs = new Map<string, string>();
    const handlers = lib.matchAll(
      /pub fn\s+(\w+)\([\s\S]*?_ctx:\s*Context<(\w+)>/g
    );

    for (const [, handlerName, accountsStruct] of handlers) {
      structs.set(accountsStruct, handlerName);
    }

    return structs;
  };

  const getInstructionName = (
    fn: string,
    instructionAccountsStructs: Map<string, string>
  ) => {
    const match =
      /_\$LT\$bench\.\.(\w+)\$u20\$as\$u20\$anchor_lang\.\.Accounts(?:\$LT\$bench\.\.\w+Bumps\$GT\$)?\$GT\$12try_accounts17h/.exec(
        fn
      );
    if (!match) return;

    return instructionAccountsStructs.get(match[1]);
  };

  const parseStackSizeSection = (output: string) => {
    const bytes: number[] = [];

    for (const line of output.split("\n")) {
      const tokens = line.trim().split(/\s+/).slice(1);
      for (const token of tokens) {
        if (!/^[\da-f]+$/i.test(token) || token.length % 2) break;
        for (let i = 0; i < token.length; i += 2) {
          bytes.push(parseInt(token.slice(i, i + 2), 16));
        }
      }
    }

    const entries = new Map<number, number>();
    for (let offset = 0; offset < bytes.length; ) {
      if (offset + 8 > bytes.length) break;

      let address = 0;
      for (let i = 0; i < 8; i++) {
        address += bytes[offset++] * 2 ** (8 * i);
      }
      if (address > 0 && address % 2 ** 32 === 0) {
        address /= 2 ** 32;
      }

      let shift = 0;
      let size = 0;
      while (offset < bytes.length) {
        const byte = bytes[offset++];
        size += (byte & 0x7f) * 2 ** shift;
        if ((byte & 0x80) === 0) break;
        shift += 7;
      }

      entries.set(address, size);
    }

    return entries;
  };

  const parseSymbols = (output: string) => {
    const symbols = new Map<number, string[]>();

    for (const line of output.split("\n")) {
      const match = /^([\da-f]+)\s+\S+\s+F\s+\.text\s+[\da-f]+\s+(.+)$/i.exec(
        line
      );
      if (!match) continue;

      const address = parseInt(match[1], 16);
      const symbol = match[2].replace(/^\.\w+\s+/, "");
      const symbolNames = symbols.get(address) ?? [];
      symbolNames.push(symbol);
      symbols.set(address, symbolNames);
    }

    return symbols;
  };

  const parseStackSizes = (
    stackSizeOutput: string,
    symbolOutput: string,
    instructionAccountsStructs: Map<string, string>
  ) => {
    const stackSizes = parseStackSizeSection(stackSizeOutput);
    const symbols = parseSymbols(symbolOutput);
    const parsedStackMemory: StackMemory = {};

    for (const [address, size] of stackSizes) {
      for (const fn of symbols.get(address) ?? []) {
        const ixName = getInstructionName(fn, instructionAccountsStructs);
        if (!ixName) continue;

        if (parsedStackMemory[ixName] !== undefined) {
          throw new Error(`Duplicate stack memory measurement for ${ixName}`);
        }
        parsedStackMemory[ixName] = size;
      }
    }

    return parsedStackMemory;
  };

  it("Measure stack memory usage", async () => {
    const bench = await BenchData.open();
    const version = getVersionFromArgs();
    const platformToolsVersion = bench.get(version).platformToolsVersion;
    const platformToolsMinor = Number(platformToolsVersion.split(".")[1]);
    const platformToolsDirectory =
      platformToolsMinor < 37 ? "sbf-tools" : "platform-tools";
    const programPath = path.join(
      "target",
      `${platformToolsMinor < 44 ? "sbf" : "sbpf"}-solana-solana`,
      "release",
      "bench.so"
    );
    const llvmObjdumpPath = path.join(
      os.homedir(),
      ".cache",
      "solana",
      platformToolsVersion,
      platformToolsDirectory,
      "llvm",
      "bin",
      "llvm-objdump"
    );
    const instructionAccountsStructs = await getInstructionAccountsStructs();

    const stackSizeResult = spawn(
      llvmObjdumpPath,
      ["-s", "-j", ".stack_sizes", programPath],
      {
        throwOnError: {
          msg: `Failed to read stack size metadata from ${programPath}.`,
        },
      }
    );
    const symbolResult = spawn(llvmObjdumpPath, ["-t", programPath], {
      throwOnError: {
        msg: `Failed to read symbols from ${programPath}.`,
      },
    });
    const parsedStackMemory = parseStackSizes(
      stackSizeResult.stdout.toString(),
      symbolResult.stdout.toString(),
      instructionAccountsStructs
    );

    if (!Object.keys(parsedStackMemory).length) {
      throw new Error(`No stack size metadata was found in ${programPath}.`);
    }

    const missingHandlers = [...instructionAccountsStructs.values()].filter(
      (handlerName) => parsedStackMemory[handlerName] === undefined
    );
    if (missingHandlers.length) {
      throw new Error(
        `Missing stack size metadata for handlers: ${missingHandlers.join(
          ", "
        )}.`
      );
    }

    Object.assign(stackMemory, parsedStackMemory);
  });

  after(async () => {
    if (!Object.keys(stackMemory).length) {
      throw new Error("No stack memory measurements were collected.");
    }

    const bench = await BenchData.open();
    await bench.update({ stackMemory });
  });
});
