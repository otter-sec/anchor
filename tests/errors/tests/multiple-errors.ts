import { spawnSync } from "child_process";

describe("multiple-errors", () => {
  it("Returns 'multiple errors' error on builds", () => {
    const result = spawnSync("anchor", [
      "idl",
      "build",
      "-p",
      "multiple-errors",
    ]);
    if (result.status === 0) {
      throw new Error("No error on build");
    }

    const output = result.output.toString();
    if (!output.includes("Error: Multiple error definitions are not allowed")) {
      throw new Error(`Unexpected error: "${output}"`);
    }
  });
});
