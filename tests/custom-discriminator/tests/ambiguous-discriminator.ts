import fs from "fs";
import { spawnSync } from "child_process";

describe("ambiguous-discriminator", () => {
  const anchorTomlPath = "Anchor.toml";
  const anchorToml = fs.readFileSync(anchorTomlPath, { encoding: "utf8" });

  before(() => {
    fs.writeFileSync(
      anchorTomlPath,
      anchorToml.replace(
        'exclude = ["programs/ambiguous-discriminator"]',
        (match) => "#" + match
      )
    );
  });

  after(() => {
    fs.writeFileSync(anchorTomlPath, anchorToml);
  });

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
      !output.includes(
        "Error: Ambiguous discriminators for accounts `AnotherAccount` and `SomeAccount`"
      )
    ) {
      throw new Error(
        `Ambiguous discriminators did not return the expected error: "${output}"`
      );
    }
  });
});
