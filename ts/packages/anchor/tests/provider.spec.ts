import {
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
} from "@solana/web3.js";
import {
  createSolanaRpcFromTransport,
  getBase64Encoder,
  getCompiledTransactionMessageDecoder,
  getTransactionDecoder,
  RpcTransport,
  SolanaError,
  SOLANA_ERROR__INSTRUCTION_ERROR__CUSTOM,
  SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
} from "@solana/kit";
import { AnchorProvider, ProviderError, SolanaClient } from "../src/provider";
import { ProgramError, translateError } from "../src/error";
import { createWallet } from "../src/wallet";

const BLOCKHASH = "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N";

type RpcRequest = { method: string; params: unknown[] };
type Responder = (request: RpcRequest) => unknown;

/**
 * A provider over a mock HTTP transport: `responders` maps RPC method names
 * to functions returning the JSON-RPC `result` (or `{ error }` envelopes),
 * and every request is recorded in `requests`.
 */
function mockProvider(responders: Record<string, Responder>) {
  const wallet = Keypair.generate();
  const requests: RpcRequest[] = [];
  const transport: RpcTransport = async ({ payload }) => {
    const request = payload as RpcRequest & { id: string; jsonrpc: string };
    requests.push(request);
    const responder = responders[request.method];
    if (!responder) {
      throw new Error(`Unexpected RPC call: ${request.method}`);
    }
    const result = responder(request);
    if (typeof result === "object" && result !== null && "error" in result) {
      return result as any;
    }
    return { jsonrpc: "2.0", id: request.id, result } as any;
  };
  const provider = new AnchorProvider(
    {
      rpc: createSolanaRpcFromTransport(transport),
      rpcSubscriptions: {} as any,
    } as SolanaClient,
    createWallet(wallet.secretKey)
  );
  return { provider, wallet, requests };
}

function latestBlockhashResponse() {
  return {
    context: { slot: 1 },
    value: { blockhash: BLOCKHASH, lastValidBlockHeight: 100 },
  };
}

function simulationResponse(overrides: Record<string, unknown> = {}) {
  return {
    context: { slot: 1 },
    value: {
      err: null,
      logs: ["Program 11111111111111111111111111111111 invoke [1]"],
      accounts: null,
      unitsConsumed: 150,
      returnData: null,
      ...overrides,
    },
  };
}

function preflightFailureResponse(request: RpcRequest, err: unknown) {
  return {
    jsonrpc: "2.0",
    id: (request as any).id,
    error: {
      code: SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
      message: "Transaction simulation failed",
      data: {
        err,
        logs: ["Program log: AnchorError thrown in programs/test/src/lib.rs"],
        accounts: null,
        unitsConsumed: 0,
        returnData: null,
      },
    },
  };
}

function transferTransaction(from: PublicKey): Transaction {
  const tx = new Transaction();
  tx.add(
    SystemProgram.transfer({
      fromPubkey: from,
      toPubkey: Keypair.generate().publicKey,
      lamports: 1,
    })
  );
  return tx;
}

/** Decodes the base64 wire transaction sent as the first RPC param. */
function decodeWireTransaction(request: RpcRequest) {
  const bytes = getBase64Encoder().encode(request.params[0] as string);
  const transaction = getTransactionDecoder().decode(bytes);
  const message = getCompiledTransactionMessageDecoder().decode(
    transaction.messageBytes
  );
  return { transaction, message };
}

describe("AnchorProvider", () => {
  describe("construction", () => {
    it("exposes the wallet address as a legacy public key", () => {
      const { provider, wallet } = mockProvider({});
      expect(provider.publicKey.toBase58()).toBe(wallet.publicKey.toBase58());
      expect(provider.wallet.address).toBe(wallet.publicKey.toBase58());
    });

    it("does not expose the legacy connection when built from a Kit client", () => {
      const { provider } = mockProvider({});
      expect(() => provider.connection).toThrow(
        "constructed from cluster endpoints"
      );
    });

    it("exposes the legacy connection when built from endpoints", () => {
      const wallet = Keypair.generate();
      const provider = new AnchorProvider(
        "http://127.0.0.1:8899",
        createWallet(wallet.secretKey)
      );
      expect(provider.connection.rpcEndpoint).toBe("http://127.0.0.1:8899");
      expect(provider.rpc).toBeDefined();
      expect(provider.rpcSubscriptions).toBeDefined();
    });
  });

  describe("simulate", () => {
    it("compiles legacy transactions with the wallet as fee payer", async () => {
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: simulationResponse,
      });

      const response = await provider.simulate(
        transferTransaction(wallet.publicKey)
      );

      expect(response.logs).toHaveLength(1);
      expect(response.unitsConsumed).toBe(150n);

      const request = requests.find((r) => r.method === "simulateTransaction")!;
      const { message } = decodeWireTransaction(request);
      expect(message.lifetimeToken).toBe(BLOCKHASH);
      expect(message.staticAccounts[0]).toBe(wallet.publicKey.toBase58());
      expect(message.staticAccounts).toContain(
        SystemProgram.programId.toBase58()
      );
    });

    it("signs a fee payer that appears only as fee payer", async () => {
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: simulationResponse,
      });

      // Gas-sponsor pattern: the sponsor pays fees but is not referenced by
      // any instruction; it must still end up signing.
      const sponsor = Keypair.generate();
      const tx = transferTransaction(wallet.publicKey);
      tx.feePayer = sponsor.publicKey;
      await provider.simulate(tx, [sponsor]);

      const request = requests.find((r) => r.method === "simulateTransaction")!;
      const { transaction, message } = decodeWireTransaction(request);
      expect(message.staticAccounts[0]).toBe(sponsor.publicKey.toBase58());
      const signatures = Object.entries(transaction.signatures);
      expect(signatures).toHaveLength(2);
      for (const [, signature] of signatures) {
        expect(signature).not.toBeNull();
      }
    });

    it("signs and verifies signatures when signers are provided", async () => {
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: simulationResponse,
      });

      const extraSigner = Keypair.generate();
      const tx = transferTransaction(extraSigner.publicKey);
      tx.feePayer = wallet.publicKey;
      await provider.simulate(tx, [extraSigner]);

      const request = requests.find((r) => r.method === "simulateTransaction")!;
      expect((request.params[1] as any).sigVerify).toBe(true);

      // Both the wallet and the extra signer signed for real: the decoder
      // maps zeroed (missing) signatures to null.
      const { transaction } = decodeWireTransaction(request);
      const signatures = Object.entries(transaction.signatures);
      expect(signatures).toHaveLength(2);
      for (const [, signature] of signatures) {
        expect(signature).not.toBeNull();
      }
    });

    it("maps legacy commitment aliases to Kit commitments", async () => {
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: simulationResponse,
      });

      await provider.simulate(
        transferTransaction(wallet.publicKey),
        undefined,
        "recent"
      );
      await provider.simulate(
        transferTransaction(wallet.publicKey),
        undefined,
        "max"
      );

      const [recent, max] = requests.filter(
        (r) => r.method === "simulateTransaction"
      );
      expect((recent.params[1] as any).commitment).toBe("processed");
      // "max" maps to "finalized", which Kit omits as the server default.
      expect((max.params[1] as any).commitment).toBeUndefined();
    });

    it("requests post-simulation accounts when includeAccounts is set", async () => {
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: (request) =>
          simulationResponse({
            accounts: (request.params[1] as any).accounts.addresses.map(
              () => null
            ),
          }),
      });

      const tx = transferTransaction(wallet.publicKey);
      await provider.simulate(tx, undefined, undefined, true);

      const request = requests.find((r) => r.method === "simulateTransaction")!;
      const accountsConfig = (request.params[1] as any).accounts;
      // Every non-program account: the wallet (fee payer and sender) and
      // the recipient of the transfer.
      expect(accountsConfig.addresses).toHaveLength(2);
      expect(accountsConfig.addresses).toContain(wallet.publicKey.toBase58());
      expect(accountsConfig.addresses).not.toContain(
        SystemProgram.programId.toBase58()
      );
    });

    it("throws a SimulateError carrying logs when simulation fails", async () => {
      const { provider, wallet } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        simulateTransaction: () =>
          simulationResponse({
            err: { InstructionError: [0, { Custom: 6000 }] },
            logs: ["Program log: Error: Custom(6000)"],
          }),
      });

      const promise = provider.simulate(transferTransaction(wallet.publicKey));
      await expect(promise).rejects.toThrow('"Custom":6000');
      await expect(promise).rejects.toMatchObject({
        logs: ["Program log: Error: Custom(6000)"],
      });
    });
  });

  describe("sendAndConfirm", () => {
    it("throws a ProviderError carrying logs on preflight failure", async () => {
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        sendTransaction: (request) =>
          preflightFailureResponse(request, {
            InstructionError: [0, { Custom: 6000 }],
          }),
      });

      const promise = provider.sendAndConfirm(
        transferTransaction(wallet.publicKey)
      );
      await expect(promise).rejects.toThrow(ProviderError);
      await expect(promise).rejects.toThrow("Custom program error: #6000");
      await expect(promise).rejects.toMatchObject({
        logs: ["Program log: AnchorError thrown in programs/test/src/lib.rs"],
      });

      // No retry for non-"already processed" failures.
      expect(
        requests.filter((r) => r.method === "sendTransaction")
      ).toHaveLength(1);
    });

    it("refreshes the blockhash and retries when already processed", async () => {
      let sends = 0;
      const { provider, wallet, requests } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        getTransaction: () => null,
        sendTransaction: (request) => {
          sends += 1;
          return preflightFailureResponse(
            request,
            sends < 3
              ? "AlreadyProcessed"
              : { InstructionError: [0, { Custom: 6000 }] }
          );
        },
      });

      const promise = provider.sendAndConfirm(
        transferTransaction(wallet.publicKey)
      );
      await expect(promise).rejects.toThrow("Custom program error: #6000");

      expect(
        requests.filter((r) => r.method === "sendTransaction")
      ).toHaveLength(3);
      // One blockhash fetch per attempt.
      expect(
        requests.filter((r) => r.method === "getLatestBlockhash")
      ).toHaveLength(3);
    });

    it("recovers logs and translates errors of failed sends", async () => {
      const logs = ["Program log: Custom error"];
      const failure = new SolanaError(SOLANA_ERROR__INSTRUCTION_ERROR__CUSTOM, {
        code: 6000,
        index: 0,
      });
      const { provider, wallet } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
        sendTransaction: () => {
          throw failure;
        },
        getTransaction: () => ({
          slot: 2,
          meta: { logMessages: logs, err: { InstructionError: [0, {}] } },
          transaction: ["", "base64"],
        }),
      });

      const promise = provider.sendAndConfirm(
        transferTransaction(wallet.publicKey)
      );
      await expect(promise).rejects.toThrow(ProviderError);
      await expect(promise).rejects.toMatchObject({ logs });

      // The Kit error survives in the cause chain, so translateError can
      // resolve the custom error code without message scraping.
      const err = await promise.catch((e) => e);
      const translated = translateError(err, new Map([[6000, "Custom error"]]));
      expect(translated).toBeInstanceOf(ProgramError);
      expect(translated.code).toBe(6000);
      expect(translated.msg).toBe("Custom error");
    });

    it("rejects durable nonce transactions explicitly", async () => {
      const { provider, wallet } = mockProvider({
        getLatestBlockhash: latestBlockhashResponse,
      });

      const tx = transferTransaction(wallet.publicKey);
      tx.nonceInfo = {
        nonce: BLOCKHASH,
        nonceInstruction: SystemProgram.nonceAdvance({
          noncePubkey: Keypair.generate().publicKey,
          authorizedPubkey: wallet.publicKey,
        }),
      };

      await expect(provider.sendAndConfirm(tx)).rejects.toThrow(
        "Durable nonce transactions are not supported"
      );
    });
  });
});
