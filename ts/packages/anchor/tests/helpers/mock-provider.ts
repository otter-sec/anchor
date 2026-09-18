import { Keypair } from "@solana/web3.js";
import {
  AccountRole,
  Address,
  address,
  appendTransactionMessageInstruction,
  Blockhash,
  createSolanaRpcFromTransport,
  createTransactionMessage,
  getBase58Decoder,
  getBase64Encoder,
  getCompiledTransactionMessageDecoder,
  getStructCodec,
  getTransactionDecoder,
  getU32Codec,
  getU64Codec,
  Instruction,
  ReadonlyUint8Array,
  RpcSubscriptions,
  RpcTransport,
  SolanaRpcSubscriptionsApi,
  SOLANA_ERROR__JSON_RPC__SERVER_ERROR_SEND_TRANSACTION_PREFLIGHT_FAILURE,
  TransactionSigner,
} from "@solana/kit";
import {
  AnchorProvider,
  ConfirmOptions,
  SolanaClient,
  WalletSigner,
} from "../../src/provider";
import { createWallet } from "../../src/wallet";

export const BLOCKHASH =
  "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N" as Blockhash;
export const SYSTEM_PROGRAM = address("11111111111111111111111111111111");

export type RpcRequest = { method: string; params: unknown[] };
export type Responder = (request: RpcRequest) => unknown;

/**
 * A provider over a mock HTTP transport: `responders` maps RPC method names
 * to functions returning the JSON-RPC `result` (or `{ error }` envelopes),
 * and every request is recorded in `requests`.
 */
export function mockProvider(
  responders: Record<string, Responder>,
  options: { wallet?: WalletSigner; opts?: ConfirmOptions } = {}
) {
  const keypair = Keypair.generate();
  const wallet = options.wallet ?? createWallet(keypair.secretKey);
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
      rpcSubscriptions: confirmingSubscriptions(),
    } as SolanaClient,
    wallet,
    options.opts
  );
  return { provider, wallet, requests };
}

/**
 * Subscriptions confirming every signature immediately and never reporting
 * a block height exceedence or a nonce account change, so that sent
 * transactions confirm.
 */
function confirmingSubscriptions(): RpcSubscriptions<SolanaRpcSubscriptionsApi> {
  async function* confirmed() {
    yield { context: { slot: 1n }, value: { err: null } };
  }
  const never = {
    [Symbol.asyncIterator]: () => ({
      next: () => new Promise<never>(() => {}),
    }),
  };
  return {
    signatureNotifications: () => ({ subscribe: async () => confirmed() }),
    slotNotifications: () => ({ subscribe: async () => never }),
    accountNotifications: () => ({ subscribe: async () => never }),
  } as unknown as RpcSubscriptions<SolanaRpcSubscriptionsApi>;
}

/** Responders needed for a transaction to be sent and confirmed. */
export function confirmingResponders(): Record<string, Responder> {
  return {
    getLatestBlockhash: latestBlockhashResponse,
    getEpochInfo: () => ({ absoluteSlot: 1, blockHeight: 1 }),
    getSignatureStatuses: () => ({ context: { slot: 1 }, value: [null] }),
    sendTransaction: (request) =>
      signatureBase58(decodeWireTransaction(request).transaction),
  };
}

/**
 * Responds to the nonce account lookup Kit's durable nonce confirmation
 * performs: the requested 32-byte slice holding the nonce value, base58.
 */
export function nonceAccountResponse(nonce: string): Responder {
  return () => ({
    context: { slot: 1 },
    value: {
      data: [nonce, "base58"],
      executable: false,
      lamports: 1_500_000,
      owner: SYSTEM_PROGRAM,
      rentEpoch: 0,
      space: 80,
    },
  });
}

export function latestBlockhashResponse() {
  return {
    context: { slot: 1 },
    value: { blockhash: BLOCKHASH, lastValidBlockHeight: 100 },
  };
}

export function simulationResponse(overrides: Record<string, unknown> = {}) {
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

export function preflightFailureResponse(request: RpcRequest, err: unknown) {
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

/** A system program transfer of one lamport from `from` to a fresh account. */
export function transferInstruction(from: Address): Instruction {
  return {
    programAddress: SYSTEM_PROGRAM,
    accounts: [
      { address: from, role: AccountRole.WRITABLE_SIGNER },
      { address: randomAddress(), role: AccountRole.WRITABLE },
    ],
    data: getStructCodec([
      ["instruction", getU32Codec()],
      ["lamports", getU64Codec()],
    ]).encode({ instruction: 2, lamports: 1n }),
  };
}

export function transferMessage(from: Address) {
  return appendTransactionMessageInstruction(
    transferInstruction(from),
    createTransactionMessage({ version: 0 })
  );
}

export function randomAddress(): Address {
  return address(Keypair.generate().publicKey.toBase58());
}

export function randomSigner(): {
  signer: TransactionSigner;
  address: Address;
} {
  const keypair = Keypair.generate();
  const signer = createWallet(keypair.secretKey);
  return { signer, address: signer.address };
}

/** Decodes the base64 wire transaction sent as the first RPC param. */
export function decodeWireTransaction(request: RpcRequest) {
  const bytes = getBase64Encoder().encode(request.params[0] as string);
  const transaction = getTransactionDecoder().decode(bytes);
  const message = getCompiledTransactionMessageDecoder().decode(
    transaction.messageBytes
  );
  return { transaction, message };
}

/** The first (fee payer) signature of a transaction, base58 encoded. */
export function signatureBase58(transaction: {
  signatures: Record<string, ReadonlyUint8Array | null>;
}): string {
  return getBase58Decoder().decode(Object.values(transaction.signatures)[0]!);
}

/**
 * Asserts the transaction has the given number of signature slots and that
 * none is left unsigned (the decoder maps zeroed signatures to null).
 */
export function expectFullySigned(
  transaction: { signatures: Record<string, ReadonlyUint8Array | null> },
  count: number
) {
  const signatures = Object.values(transaction.signatures);
  expect(signatures).toHaveLength(count);
  for (const signature of signatures) {
    expect(signature).not.toBeNull();
  }
}
