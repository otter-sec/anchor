import { isBrowser } from "./utils/common.js";

export { default as BN } from "bn.js";
export * as web3 from "@solana/web3.js";
export {
  default as Provider,
  getProvider,
  setProvider,
  AnchorProvider,
} from "./provider.js";
export * from "./error.js";
export { Instruction } from "./coder/borsh/instruction.js";
export * from "./idl.js";
export { CustomAccountResolver } from "./program/accounts-resolver.js";

export * from "./coder/index.js";
export * as utils from "./utils/index.js";
export * from "./program/index.js";
export * from "./native/index.js";

// `Wallet` is a real top-level re-export. `nodewallet.ts` only references Node
// globals lazily inside `Wallet.local()`, so a static import is safe in any
// environment (calls to `.local()` throw in browsers, as intended).
export { default as Wallet } from "./nodewallet.js";

// `workspace` depends on Node-only modules (`fs`, `path`, `child_process`,
// `toml`) at module load, so we can't statically re-export it without pulling
// those into the browser bundle. Instead, expose a real Proxy that:
//   - In CJS: the `exports.workspace` assignment below installs the real
//     implementation; the Proxy is never reached.
//   - In ESM Node: the Proxy throws a clear, actionable error pointing users
//     at a CJS-via-createRequire workaround, instead of silently being
//     `undefined` and showing up downstream as `Cannot read property of undefined`.
//   - In browsers: the Proxy throws "Workspaces aren't available in the browser".
export const workspace: any = new Proxy(
  {},
  {
    get(_target, prop: string | symbol) {
      if (isBrowser) {
        throw new Error("Workspaces aren't available in the browser");
      }
      throw new Error(
        "`workspace` is only available in the CommonJS build of " +
          "@anchor-lang/core. From an ESM module, load it via:\n" +
          "  import { createRequire } from 'module';\n" +
          "  const require = createRequire(import.meta.url);\n" +
          "  const { workspace } = require('@anchor-lang/core');\n" +
          `Tried to access workspace.${String(prop)}.`
      );
    },
  }
);

// CJS-only override: replace the proxy export with the real implementation.
// `exports`/`require` don't exist in native ESM (dist/esm) or in bundlers that
// consume the "module" field, so the guard makes the ESM build a no-op
// (preserving the Proxy above) instead of crashing with a ReferenceError that
// would take BN and every other re-export down with it.
if (!isBrowser && typeof exports !== "undefined") {
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  exports.workspace = require("./workspace.js").default;
}
