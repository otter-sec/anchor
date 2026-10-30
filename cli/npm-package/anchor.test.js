const assert = require("node:assert/strict");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const PACKAGE_VERSION = `anchor-cli ${require("./package.json").version}`;

function makePackage() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "anchor-npm-"));
  const wrapperPath = path.join(dir, "anchor.js");
  const preloadPath = path.join(dir, "mock-os.js");

  fs.copyFileSync(path.join(__dirname, "anchor.js"), wrapperPath);
  fs.copyFileSync(path.join(__dirname, "package.json"), path.join(dir, "package.json"));
  fs.chmodSync(wrapperPath, 0o755);

  fs.writeFileSync(
    preloadPath,
    `
const os = require("node:os");
os.arch = () => "arm64";
os.platform = () => "darwin";
`
  );

  return { dir, preloadPath, wrapperPath };
}

function writeFakeAnchor(filePath, { version = PACKAGE_VERSION, runOutput, runExit = 0 } = {}) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(
    filePath,
    [
      "#!/bin/sh",
      'if [ "$1" = "--version" ]; then',
      `  printf '%s\\n' ${JSON.stringify(version)}`,
      "  exit 0",
      "fi",
      runOutput ? `printf '%s\\n' ${JSON.stringify(runOutput)}` : ":",
      `exit ${runExit}`,
      "",
    ].join("\n"),
    { mode: 0o755 }
  );
}

function runWrapper(npmPackage, { args = [], pathValue = "" } = {}) {
  return spawnSync(
    process.execPath,
    ["--require", npmPackage.preloadPath, npmPackage.wrapperPath, ...args],
    {
      encoding: "utf8",
      env: { PATH: pathValue },
    }
  );
}

test("exits non-zero if global anchor fallback is missing", () => {
  const npmPackage = makePackage();

  const result = runWrapper(npmPackage);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Trying globally installed anchor/);
  assert.match(result.stderr, /Could not find globally installed anchor/);
});

test("exits non-zero if global anchor fallback has the wrong version", () => {
  const npmPackage = makePackage();
  const globalBin = fs.mkdtempSync(path.join(os.tmpdir(), "anchor-global-"));
  writeFakeAnchor(path.join(globalBin, "anchor"), {
    version: "anchor-cli 0.0.0",
  });

  const result = runWrapper(npmPackage, { pathValue: globalBin });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Trying globally installed anchor/);
  assert.match(result.stderr, /Globally installed anchor version is not correct/);
});

test("runs matching global anchor fallback", () => {
  const npmPackage = makePackage();
  const globalBin = fs.mkdtempSync(path.join(os.tmpdir(), "anchor-global-"));
  writeFakeAnchor(path.join(globalBin, "anchor"), {
    runExit: 7,
    runOutput: "global anchor ran",
  });

  const result = runWrapper(npmPackage, {
    args: ["build"],
    pathValue: globalBin,
  });

  assert.equal(result.status, 7);
  assert.match(result.stderr, /Trying globally installed anchor/);
  assert.match(result.stdout, /global anchor ran/);
});
