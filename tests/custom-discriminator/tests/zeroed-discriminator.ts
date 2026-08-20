import fs from "fs";
import { spawnSync } from "child_process";

describe("zeroed-discriminator", () => {
  const anchorTomlPath = "Anchor.toml";
  const anchorToml = fs.readFileSync(anchorTomlPath, { encoding: "utf8" });

  before(() => {
    fs.writeFileSync(
      anchorTomlPath,
      anchorToml.replace(
        'exclude = ["programs/ambiguous-discriminator", "programs/zeroed-discriminator"]',
        'exclude = ["programs/ambiguous-discriminator"]'
      )
    );
  });

  after(() => {
    fs.writeFileSync(anchorTomlPath, anchorToml);
  });

  it("rejects zeroed account discriminators on no-idl builds", () => {
    const result = spawnSync("anchor", [
      "build",
      "--no-idl",
      "--ignore-keys",
      "-p",
      "zeroed-discriminator",
    ]);
    if (result.status === 0) {
      throw new Error("Zeroed discriminator build unexpectedly succeeded");
    }

    const output = result.output.toString();
    if (
      !output.includes("all-zero or empty discriminators are not supported")
    ) {
      throw new Error(
        `Zeroed discriminator build did not return the expected error: "${output}"`
      );
    }
  });
});
