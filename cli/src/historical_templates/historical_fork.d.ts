import type { AnchorProvider, web3 } from "@anchor-lang/core";

export declare class HistoricalForkError extends Error {
  readonly code: string;
}

export declare class HistoricalFork {
  /**
   * Starts a fork after a transaction signature or slot chosen in test code.
   */
  static start(options: { after: string; dataDir?: string; startupTimeoutMs?: number }): Promise<HistoricalFork>;
  /** Raw endpoints; independently created clients do not inherit helper lifetime guards. */
  readonly url: string;
  readonly wsUrl: string;
  readonly surfnetId: string;
  readonly after: string;
  readonly pid: number;
  readonly directory: string;
  /** Uses one disposable key per fork. HTTP calls refuse after child exit;
   * WebSocket connection attempts are blocked with a diagnostic. Registration
   * may still return a subscription id. Never supply a real funded wallet:
   * durable-nonce transactions may remain valid until the on-chain nonce advances. */
  provider(keypair?: web3.Keypair, options?: web3.ConfirmOptions): AnchorProvider;
  /**
   * An RPC call to this verified fork, including explicit `surfnet_*` controls.
   */
  rpc(method: string, params?: unknown[]): Promise<any>;
  /** Defines an account in this fork only. */
  setAccount(pubkey: web3.PublicKey | string, account?: { lamports?: number; owner?: web3.PublicKey | string; data?: Uint8Array; executable?: boolean }): Promise<unknown>;
  /**
   * Fast ELF setup via Surfpool's program-writing helper; not loader deployment.
   */
  installProgram(programId: web3.PublicKey | string, soPath: string, authority?: web3.PublicKey): Promise<void>;
  /** Explicit local slots; the historical clock never advances on its own. */
  advance(slots?: number): Promise<unknown>;
  /** Stops this fork's own child process. */
  close(): Promise<void>;
}
