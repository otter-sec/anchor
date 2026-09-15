import { isBrowser } from "./utils/common.js";

export { default as BN } from "bn.js";
export * as web3 from "@solana/web3.js";
export { some, none } from "@solana/kit";
export type { Option, Some, None, TransactionSigner } from "@solana/kit";
export {
  default as Provider,
  getProvider,
  setProvider,
  AnchorProvider,
  ProviderError,
  SimulateError,
} from "./provider.js";
export type {
  SolanaClient,
  ClusterEndpoints,
  ConfirmOptionsWithBlockhash,
  WalletSigner,
} from "./provider.js";
export { createWallet, createLocalWallet } from "./wallet.js";
export * from "./error.js";
export { Instruction } from "./coder/borsh/instruction.js";
export * from "./idl.js";
export { CustomAccountResolver } from "./program/accounts-resolver.js";

export * from "./coder/index.js";
export * as utils from "./utils/index.js";
export * from "./program/index.js";
export * from "./native/index.js";

export declare const workspace: any;

if (!isBrowser) {
  exports.workspace = require("./workspace.js").default;
}
