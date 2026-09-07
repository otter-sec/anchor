import { execSync } from "child_process";
import * as fs from "fs/promises";
import path from "path";

import { BenchData, BinarySize } from "../scripts/utils";

const PROGRAM_NAME = "bench";

describe("Binary size", () => {
  const binarySize: BinarySize = {};

  it("Measure binary size", async () => {
    const output = execSync("cargo metadata --no-deps --format-version=1", {
      encoding: "utf8",
    });
    const metadata = JSON.parse(output);
    const stat = await fs.stat(
      path.join(metadata.target_directory, "deploy", `${PROGRAM_NAME}.so`)
    );
    binarySize[PROGRAM_NAME] = stat.size;
  });

  after(async () => {
    const bench = await BenchData.open();
    await bench.update({ binarySize });
  });
});
