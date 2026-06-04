import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import { resolveIdlFileName } from "../src/workspace";

describe("workspace IDL resolution", () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "anchor-idl-test-"));
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it("finds the correct IDL when the program name matches exactly", () => {
    fs.writeFileSync(path.join(tmpDir, "user_g_market.json"), "{}");
    expect(resolveIdlFileName(tmpDir, "userGMarket")).toBe(
      "user_g_market.json"
    );
  });

  it("throws a helpful error listing available IDLs when name has a typo", () => {
    // Replicates: Cargo.toml has `name = "user_g_market"` (singular)
    // but workspace references `user_g_markets` (plural)
    fs.writeFileSync(path.join(tmpDir, "user_g_market.json"), "{}");

    expect(() => resolveIdlFileName(tmpDir, "userGMarkets")).toThrowError(
      /Failed to find IDL for program `userGMarkets`/
    );
    expect(() => resolveIdlFileName(tmpDir, "userGMarkets")).toThrowError(
      /user_g_market/
    );
    expect(() => resolveIdlFileName(tmpDir, "userGMarkets")).toThrowError(
      /\[lib\]\.name.*Cargo\.toml/
    );
    expect(() => resolveIdlFileName(tmpDir, "userGMarkets")).toThrowError(
      /\#\[program\].*Rust program/
    );
    expect(() => resolveIdlFileName(tmpDir, "userGMarkets")).toThrowError(
      /Anchor\.toml/
    );
  });

  it("lists multiple available IDLs sorted alphabetically on typo", () => {
    fs.writeFileSync(path.join(tmpDir, "my_program.json"), "{}");
    fs.writeFileSync(path.join(tmpDir, "another_program.json"), "{}");

    expect(() => resolveIdlFileName(tmpDir, "missingProgram")).toThrowError(
      /another_program, my_program/
    );
  });

  it("throws a friendly error when the idl/ directory doesn't exist", () => {
    const nonExistentDir = path.join(tmpDir, "idl");
    expect(() => resolveIdlFileName(nonExistentDir, "anyProgram")).toThrowError(
      /IDL directory not found.*Did you run `anchor build`/
    );
  });

  it("throws a friendly error when the idl/ directory exists but is empty", () => {
    expect(() => resolveIdlFileName(tmpDir, "anyProgram")).toThrowError(
      /No IDL files found.*Did you run `anchor build`/
    );
  });

  it("ignores non-JSON files and directories when scanning for IDLs", () => {
    fs.writeFileSync(path.join(tmpDir, "user_g_market.json"), "{}");
    fs.writeFileSync(path.join(tmpDir, "readme.txt"), "ignore me");
    fs.mkdirSync(path.join(tmpDir, "user_g_market"));

    // Should still find the correct JSON IDL and not trip over the txt or dir
    expect(resolveIdlFileName(tmpDir, "userGMarket")).toBe(
      "user_g_market.json"
    );
  });

  it("rethrows non-ENOENT errors from readdirSync unchanged (e.g. EACCES)", () => {
    const accessError = Object.assign(
      new Error("EACCES: permission denied, scandir '/protected'"),
      { code: "EACCES" }
    );

    // `import * as fs` produces a getter-only namespace; require() gives the
    // raw mutable CJS module object needed to patch the property.
    const fsModule = require("fs");
    const original = fsModule.readdirSync;
    fsModule.readdirSync = () => {
      throw accessError;
    };
    try {
      expect(() => resolveIdlFileName(tmpDir, "anyProgram")).toThrow(
        accessError
      );
    } finally {
      fsModule.readdirSync = original;
    }
  });

  it("does not list non-JSON files in the available programs error", () => {
    fs.writeFileSync(path.join(tmpDir, "user_g_market.json"), "{}");
    fs.writeFileSync(path.join(tmpDir, "readme.txt"), "ignore me");

    expect(() => resolveIdlFileName(tmpDir, "missingProgram")).toThrowError(
      /user_g_market/
    );
    expect(() => resolveIdlFileName(tmpDir, "missingProgram")).not.toThrowError(
      /readme/
    );
  });
});
