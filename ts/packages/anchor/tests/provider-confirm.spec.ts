import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";

import { AnchorProvider, Wallet } from "../src";
import { inspect } from "util";
import { format, parse } from "url";

// @solana/web3.js can reject `confirmTransaction` with a bare `TransactionError`
// read from an already failed status. `sendAndConfirm` must recover the logs
// like any failed confirmation, but only when the signature's status proves
// that failure. Any other rejection stays untouched, and nothing is resent.

const BLOCKHASH = "GfVcyD5tT6Aua4yzGiVy5gM2y2qXmZRr5GnMaCu2FQVc";
const CUSTOM_6000 = { InstructionError: [0, { Custom: 6000 }] };
const LOGS = [
  "Program 59eu89dUdFWrGzCzT6WzjpPmn9VnwmG4a3NwFmxHu8MN invoke [1]",
  "Program log: AnchorError thrown in programs/demo/src/instructions/increment.rs:13. Error Code: Unauthorized. Error Number: 6000. Error Message: Only the counter authority can update this counter.",
  "Program 59eu89dUdFWrGzCzT6WzjpPmn9VnwmG4a3NwFmxHu8MN failed: custom program error: 0x1770",
];

function fixture(
  rejection: unknown,
  statusErr: unknown,
  statusFailure?: Error,
  rpcEndpoint = "http://127.0.0.1:8899"
) {
  const calls = { send: 0, status: 0, transaction: 0 };
  const connection = {
    rpcEndpoint,
    commitment: "processed",
    sendRawTransaction: async () => {
      calls.send++;
      return "5zRthhgMVjWVzTutDKpAtWryqjMabizAJPKjPVoUCcpkx7vQEDwQZgmE3EpQJxuPxmLtDP7fm6MTaHonf9kcypos";
    },
    confirmTransaction: async () => {
      throw rejection;
    },
    getSignatureStatus: async () => {
      calls.status++;
      if (statusFailure) throw statusFailure;
      return { context: { slot: 1 }, value: { err: statusErr } };
    },
    getTransaction: async () => {
      calls.transaction++;
      return { meta: { logMessages: LOGS } };
    },
  } as unknown as Connection;
  const payer = Keypair.generate();
  const wallet: Wallet = {
    publicKey: payer.publicKey,
    signTransaction: async (tx: any) => {
      tx.sign(payer);
      return tx;
    },
    signAllTransactions: async (txs: any[]) => txs,
    payer,
  } as any;
  const tx = new Transaction({
    feePayer: payer.publicKey,
    recentBlockhash: BLOCKHASH,
  }).add(
    SystemProgram.transfer({
      fromPubkey: payer.publicKey,
      toPubkey: new PublicKey(BLOCKHASH),
      lamports: 1,
    })
  );
  return {
    provider: new AnchorProvider(connection, wallet, {
      commitment: "processed",
    }),
    tx,
    calls,
  };
}

describe("sendAndConfirm with a bare TransactionError rejection", () => {
  it.each([
    "https:\\/\\/provider.invalid\\/v2\\/not-a-real-api-key",
    "/v2/not-a-real-api-key",
    "Invalid API key: not-a-real-api-key",
    "getaddrinfo ENOTFOUND not-a-real-api-key.provider.invalid",
  ])("omits arbitrary transport diagnostics: %s", async (detail) => {
    const rejection = { reason: detail };
    const { provider, tx, calls } = fixture(rejection, null, new Error(detail));
    const logged = jest.spyOn(console, "error").mockImplementation(() => {});
    try {
      await expect(provider.sendAndConfirm(tx)).rejects.toBe(rejection);
      expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
      expect(logged).toHaveBeenCalledTimes(1);
      expect(logged.mock.calls[0]).toEqual([
        "RPC_STATUS_UNAVAILABLE: could not read transaction status after a confirmation rejection; original error preserved.",
      ]);
    } finally {
      logged.mockRestore();
    }
  });
  it.each(
    [
      "https://not-a-real-api-key.provider.invalid/",
      "https://NOT-A-REAL-API-KEY.provider.invalid/",
      "https://not-a-real-api-key.bücher.invalid/",
    ].flatMap((endpoint) =>
      [false, true].map((bareHost) => [endpoint, bareHost] as [string, boolean])
    )
  )(
    "redacts a credential-bearing DNS hostname %s (bare rejection: %s)",
    async (endpoint, bareHost) => {
      const hostname = new URL(endpoint).hostname;
      const rejection = bareHost
        ? `getaddrinfo ENOTFOUND ${hostname.toUpperCase()}`
        : CUSTOM_6000;
      const { provider, tx, calls } = fixture(
        rejection,
        null,
        new Error(
          `request to ${endpoint} failed, reason: getaddrinfo ENOTFOUND ${hostname}`
        ),
        endpoint
      );
      const logged = jest.spyOn(console, "error").mockImplementation(() => {});
      try {
        await expect(provider.sendAndConfirm(tx)).rejects.toBe(rejection);
        expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
        expect(logged).toHaveBeenCalledTimes(1);
        const diagnostic = inspect(logged.mock.calls);
        expect(diagnostic).toContain("RPC_STATUS_UNAVAILABLE");
        expect(diagnostic.toLowerCase()).not.toContain("not-a-real-api-key");
        expect(diagnostic.toLowerCase()).not.toContain(hostname.toLowerCase());
      } finally {
        logged.mockRestore();
      }
    }
  );

  it.each(["not a URL", "file:///not-a-real-api-key"])(
    "keeps the rejection and omits transport details for invalid endpoint %s",
    async (endpoint) => {
      const { provider, tx, calls } = fixture(
        CUSTOM_6000,
        null,
        new Error("getaddrinfo ENOTFOUND not-a-real-api-key.provider.invalid"),
        endpoint
      );
      const logged = jest.spyOn(console, "error").mockImplementation(() => {});
      try {
        await expect(provider.sendAndConfirm(tx)).rejects.toBe(CUSTOM_6000);
        expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
        expect(logged).toHaveBeenCalledTimes(1);
        const diagnostic = inspect(logged.mock.calls);
        expect(diagnostic).toContain("RPC_STATUS_UNAVAILABLE");
        expect(diagnostic).not.toContain("not-a-real-api-key");
      } finally {
        logged.mockRestore();
      }
    }
  );

  it.each(
    [
      "http://dummyuser:not-a-real-api-key$PASS@127.0.0.1:1/",
      "http://dummyuser:not-a-real-api-key&PASS@127.0.0.1:1/",
      "http://127.0.0.1:1/v2/not-a-real-api-key^KEY",
      "http://127.0.0.1:1/v2/not-a-real-api-key|KEY",
      "http://127.0.0.1:1/?api-key=not-a-real-api-key^KEY",
      "http://127.0.0.1:1/?api-key=not-a-real-api-key{KEY}",
    ].flatMap((endpoint) =>
      [false, true].map((bareUrl) => [endpoint, bareUrl] as [string, boolean])
    )
  )(
    "redacts a legacy-encoded credential-bearing endpoint %s (bare URL: %s)",
    async (endpoint, bareUrl) => {
      const reported = format(parse(endpoint));
      expect(reported).not.toBe(endpoint);
      expect(reported).not.toBe(new URL(endpoint).href);
      const rejection = bareUrl
        ? `transport failed at ${reported}`
        : CUSTOM_6000;
      const { provider, tx, calls } = fixture(
        rejection,
        null,
        new Error(`request to ${reported} failed, reason: ECONNREFUSED`),
        endpoint
      );
      const logged = jest.spyOn(console, "error").mockImplementation(() => {});
      try {
        await expect(provider.sendAndConfirm(tx)).rejects.toBe(rejection);
        expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
        const diagnostic = inspect(logged.mock.calls);
        expect(diagnostic).toContain("RPC_STATUS_UNAVAILABLE");
        expect(diagnostic).not.toContain("not-a-real-api-key");
      } finally {
        logged.mockRestore();
      }
    }
  );

  it.each(
    [
      "http://127.0.0.1:1?api_key=not-a-real-api-key",
      "http://LOCALHOST:1/v2/not-a-real-api-key?access_token=another-fake-key",
      "http://127.0.0.1:1/a b/not-a-real-api-key",
      "https://provider.invalid:443/v2/not-a-real-api-key",
    ].flatMap((endpoint) =>
      [false, true].map((bareUrl) => [endpoint, bareUrl] as [string, boolean])
    )
  )(
    "redacts a normalized credential-bearing endpoint %s (bare URL: %s)",
    async (endpoint, bareUrl) => {
      const normalized = new URL(endpoint).href;
      expect(normalized).not.toBe(endpoint);
      const rejection = bareUrl
        ? `transport failed at ${normalized}`
        : CUSTOM_6000;
      const { provider, tx, calls } = fixture(
        rejection,
        null,
        new Error(`request to ${normalized} failed`),
        endpoint
      );
      const logged = jest.spyOn(console, "error").mockImplementation(() => {});
      try {
        await expect(provider.sendAndConfirm(tx)).rejects.toBe(rejection);
        expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
        expect(logged).toHaveBeenCalledTimes(1);
        const diagnostic = inspect(logged.mock.calls);
        expect(diagnostic).toContain("RPC_STATUS_UNAVAILABLE");
        expect(diagnostic).not.toContain("not-a-real-api-key");
        expect(diagnostic).not.toContain("another-fake-key");
      } finally {
        logged.mockRestore();
      }
    }
  );

  it.each([false, true])(
    "redacts a credential-bearing endpoint in status diagnostics (bare URL: %s)",
    async (bareUrl) => {
      const endpoint = "https://provider.invalid/v2/not-a-real-api-key";
      const rejection = bareUrl
        ? `transport failed at ${endpoint}`
        : CUSTOM_6000;
      const transport = new Error(`request to ${endpoint} failed`);
      const { provider, tx, calls } = fixture(
        rejection,
        null,
        transport,
        endpoint
      );
      const logged = jest.spyOn(console, "error").mockImplementation(() => {});
      try {
        await expect(provider.sendAndConfirm(tx)).rejects.toBe(rejection);
        expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
        expect(logged).toHaveBeenCalledTimes(1);
        const diagnostic = inspect(logged.mock.calls);
        expect(diagnostic).toContain("RPC_STATUS_UNAVAILABLE");
        expect(diagnostic).not.toContain("not-a-real-api-key");
      } finally {
        logged.mockRestore();
      }
    }
  );

  it("recovers the program logs when the status proves the same failure", async () => {
    const { provider, tx, calls } = fixture(CUSTOM_6000, CUSTOM_6000);
    const error: any = await provider.sendAndConfirm(tx).catch((e) => e);
    expect(error.logs).toEqual(LOGS);
    expect(calls).toEqual({ send: 1, status: 1, transaction: 1 });
  });

  it("rethrows a bare value the status does not confirm", async () => {
    const { provider, tx, calls } = fixture(CUSTOM_6000, null);
    await expect(provider.sendAndConfirm(tx)).rejects.toBe(CUSTOM_6000);
    expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
  });

  it("keeps the original rejection and logs why when its status cannot be read", async () => {
    const transport = new Error("socket hang up");
    const { provider, tx, calls } = fixture(CUSTOM_6000, null, transport);
    const logged = jest.spyOn(console, "error").mockImplementation(() => {});
    try {
      await expect(provider.sendAndConfirm(tx)).rejects.toBe(CUSTOM_6000);
      expect(calls).toEqual({ send: 1, status: 1, transaction: 0 });
      expect(logged).toHaveBeenCalledTimes(1);
      expect(inspect(logged.mock.calls[0])).toContain("RPC_STATUS_UNAVAILABLE");
      expect(inspect(logged.mock.calls[0])).not.toContain("socket hang up");
    } finally {
      logged.mockRestore();
    }
  });

  it("leaves transport errors untouched and resends nothing", async () => {
    const transport = new Error("socket hang up");
    const { provider, tx, calls } = fixture(transport, CUSTOM_6000);
    await expect(provider.sendAndConfirm(tx)).rejects.toBe(transport);
    expect(calls).toEqual({ send: 1, status: 0, transaction: 0 });
  });
});
