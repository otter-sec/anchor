module.exports = {
  preset: "ts-jest/presets/default",
  testEnvironment: "node",
  testTimeout: 90000,
  resolver: "ts-jest-resolver",
  setupFiles: ["<rootDir>/tests/setup.js"],
  moduleNameMapper: {
    "^@solana/kit/program-client-core$":
      "<rootDir>/../../node_modules/@solana/kit/dist/program-client-core.node.cjs",
  },
};
