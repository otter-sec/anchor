import { spawnSync } from "child_process";

describe("ambiguous-discriminator", () => {
  it("Returns ambiguous discriminator error on builds", () => {
    const result = spawnSync("anchor", [
      "idl",
      "build",
      "-p",
      "ambiguous-discriminator",
    ]);
    if (result.status === 0) {
      throw new Error("Ambiguous errors did not make building the IDL fail");
    }

    const output = result.output.toString();
    if (
      !output.includes("Error: Program 'ambiguous-discriminator' not found")
    ) {
      throw new Error(
        `Ambiguous discriminators did not return the expected error: "${output}"`
      );
    }
  });
});
