/** Sync Markdown files in /bench based on the data from bench.json */

import { BenchData, BenchResult, Markdown, formatNumber } from "./utils";

(async () => {
  const bench = await BenchData.open();

  await BenchData.forEachMarkdown((markdown, fileName) => {
    const resultType = fileName
      .toLowerCase()
      .replace(".md", "")
      .replace(/_\w/g, (match) => match[1].toUpperCase()) as keyof BenchResult;

    const versions = bench.getVersions();

    // On the first version, compare with itself to update it with no changes
    versions.unshift(versions[0]);

    for (const i in versions) {
      const currentVersion = versions[i];
      if (currentVersion === "unreleased") return;

      const nextVersion = versions[+i + 1];
      const newData = bench.get(nextVersion);
      const oldData = bench.get(currentVersion);

      // Create table
      const table = Markdown.createTable();

      bench.compare({
        newResult: newData.result[resultType],
        oldResult: oldData.result[resultType],
        changeCb: ({ name, newValue, oldValue }) => {
          if (newValue === null) {
            // Deleted key
            return;
          }

          let changeText: string;
          if (oldValue === null) {
            // New key
            changeText = "N/A";
          } else {
            const delta = formatNumber(newValue - oldValue);
            const percentChange = ((newValue / oldValue - 1) * 100).toFixed(2);

            if (+percentChange > 0) {
              changeText = `🔴 **+${delta} (${percentChange}%)**`;
            } else {
              changeText = `🟢 **${delta} (${percentChange.slice(1)}%)**`;
            }
          }

          table.insert(name, formatNumber(newValue), changeText);
        },
        noChangeCb: ({ name, value }) => {
          table.insert(name, formatNumber(value), +i === 0 ? "N/A" : "-");
        },
      });

      // Update version data
      markdown.updateVersion({
        version: nextVersion,
        solanaVersion: newData.solanaVersion,
        table,
      });
    }
  });
})();
