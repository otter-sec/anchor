import nodeResolve from "@rollup/plugin-node-resolve";
import typescript from "@rollup/plugin-typescript";
import replace from "@rollup/plugin-replace";
import commonjs from "@rollup/plugin-commonjs";

const env = process.env.NODE_ENV;

export default {
  input: "src/index.ts",
  plugins: [
    commonjs(),
    nodeResolve({
      browser: true,
      extensions: [".js", ".ts"],
      dedupe: ["bn.js", "buffer"],
      preferBuiltins: false,
    }),
    typescript({
      tsconfig: "./tsconfig.base.json",
      moduleResolution: "node",
      outDir: "types",
      target: "es2019",
      outputToFilesystem: false,
    }),
    replace({
      preventAssignment: true,
      values: {
        "process.env.NODE_ENV": JSON.stringify(env),
        "process.env.ANCHOR_BROWSER": JSON.stringify(true),
      },
    }),
  ],
  external: [
    "@anchor-lang/borsh",
    "@solana/web3.js",
    "bn.js",
    "bs58",
    "buffer",
    "camelcase",
    "eventemitter3",
    "@noble/hashes/sha256",
    "pako",
    "toml",
  ],
  output: {
    file: "dist/browser/index.js",
    format: "es",
    sourcemap: true,
  },
};
