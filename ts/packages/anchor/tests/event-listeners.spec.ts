import { address, getU64Codec } from "@solana/kit";
import { Idl, Program } from "../src";
import { mockProvider } from "./helpers/mock-provider";

const PROGRAM_ADDRESS = address("Test111111111111111111111111111111111111111");
const SIGNATURE =
  "5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW";

const idl = {
  address: PROGRAM_ADDRESS,
  metadata: { name: "counter", version: "0.1.0", spec: "0.1.0" },
  instructions: [],
  events: [
    { name: "incremented", discriminator: [1, 2, 3, 4, 5, 6, 7, 8] },
    { name: "reset", discriminator: [8, 7, 6, 5, 4, 3, 2, 1] },
  ],
  types: [
    {
      name: "incremented",
      type: { kind: "struct", fields: [{ name: "count", type: "u64" }] },
    },
    { name: "reset", type: { kind: "struct", fields: [] } },
  ],
} as const satisfies Idl;
type CounterIdl = typeof idl;

function eventLog(discriminator: readonly number[], count?: bigint) {
  const data = Buffer.concat([
    Buffer.from(discriminator),
    count === undefined
      ? Buffer.alloc(0)
      : Buffer.from(getU64Codec().encode(count)),
  ]);
  return `Program data: ${data.toString("base64")}`;
}

function logsNotification(
  logs: string[],
  options: { slot?: number; err?: unknown } = {}
) {
  return {
    context: { slot: BigInt(options.slot ?? 10) },
    value: {
      err: options.err ?? null,
      logs: [
        `Program ${PROGRAM_ADDRESS} invoke [1]`,
        ...logs,
        `Program ${PROGRAM_ADDRESS} success`,
      ],
      signature: SIGNATURE,
    },
  };
}

/**
 * A `logsNotifications` mock recording every call and yielding the given
 * notifications, then hanging until aborted (like a live subscription).
 */
function logsSubscriptions(
  notifications: ReturnType<typeof logsNotification>[],
  calls: unknown[][] = [],
  aborted: AbortSignal[] = []
) {
  return {
    logsNotifications: (...args: unknown[]) => {
      calls.push(args);
      return {
        subscribe: async ({ abortSignal }: { abortSignal: AbortSignal }) => {
          aborted.push(abortSignal);
          return (async function* () {
            for (const notification of notifications) {
              if (abortSignal.aborted) return;
              yield notification;
            }
            await new Promise<void>((resolve) =>
              abortSignal.addEventListener("abort", () => resolve())
            );
          })();
        },
      };
    },
  };
}

function nextTick() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("Program.addEventListener", () => {
  it("invokes the callback for matching events with slot and signature", async () => {
    const calls: unknown[][] = [];
    const { provider } = mockProvider(
      {},
      {
        subscriptions: logsSubscriptions(
          [
            logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 5n)], {
              slot: 42,
            }),
            logsNotification([eventLog([8, 7, 6, 5, 4, 3, 2, 1])]),
            logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 6n)]),
          ],
          calls
        ),
      }
    );
    const program = new Program<CounterIdl>(idl, provider);

    const received: [bigint, bigint, string][] = [];
    const controller = new AbortController();
    program.addEventListener(
      "incremented",
      (event, slot, signature) => {
        received.push([event.count, slot, signature]);
      },
      { abortSignal: controller.signal }
    );
    await nextTick();
    controller.abort();

    expect(received).toEqual([
      [5n, 42n, SIGNATURE],
      [6n, 10n, SIGNATURE],
    ]);
    // Mentions this program, at the provider's default commitment.
    expect(calls).toEqual([
      [{ mentions: [PROGRAM_ADDRESS] }, { commitment: "confirmed" }],
    ]);
  });

  it("ignores logs of failed transactions", async () => {
    const { provider } = mockProvider(
      {},
      {
        subscriptions: logsSubscriptions([
          logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 1n)], {
            err: { InstructionError: [0, "Custom"] },
          }),
        ]),
      }
    );
    const program = new Program<CounterIdl>(idl, provider);

    const callback = jest.fn();
    const controller = new AbortController();
    program.addEventListener("incremented", callback, {
      abortSignal: controller.signal,
    });
    await nextTick();
    controller.abort();

    expect(callback).not.toHaveBeenCalled();
  });

  it("keeps listening when the callback throws", async () => {
    const { provider } = mockProvider(
      {},
      {
        subscriptions: logsSubscriptions([
          logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 1n)]),
          logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 2n)]),
        ]),
      }
    );
    const program = new Program<CounterIdl>(idl, provider);
    const controller = new AbortController();
    const failure = new Error("callback failed");

    const received: bigint[] = [];
    const errors: [unknown, { fatal: boolean }][] = [];
    program.addEventListener(
      "incremented",
      (event) => {
        received.push(event.count);
        if (event.count === 1n) throw failure;
      },
      {
        abortSignal: controller.signal,
        onError: (error, context) => errors.push([error, context]),
      }
    );
    await nextTick();
    controller.abort();

    expect(received).toEqual([1n, 2n]);
    expect(errors).toEqual([[failure, { fatal: false }]]);
  });

  it("delivers the remaining events of a batch when the callback throws", async () => {
    const { provider } = mockProvider(
      {},
      {
        subscriptions: logsSubscriptions([
          logsNotification([
            eventLog([1, 2, 3, 4, 5, 6, 7, 8], 1n),
            eventLog([1, 2, 3, 4, 5, 6, 7, 8], 2n),
          ]),
        ]),
      }
    );
    const program = new Program<CounterIdl>(idl, provider);
    const controller = new AbortController();

    const received: bigint[] = [];
    const errors: unknown[] = [];
    program.addEventListener(
      "incremented",
      (event) => {
        received.push(event.count);
        if (event.count === 1n) throw new Error("callback failed");
      },
      {
        abortSignal: controller.signal,
        onError: (error) => errors.push(error),
      }
    );
    await nextTick();
    controller.abort();

    // Both events of the transaction are delivered, one failure reported.
    expect(received).toEqual([1n, 2n]);
    expect(errors).toHaveLength(1);
  });

  it("skips log batches it cannot parse", async () => {
    const { provider } = mockProvider(
      {},
      {
        subscriptions: logsSubscriptions([
          {
            ...logsNotification([]),
            value: {
              err: null,
              // Not the `invoke [1]` line the parser expects first.
              logs: [`Program ${PROGRAM_ADDRESS} success`],
              signature: SIGNATURE,
            },
          },
          logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 3n)]),
        ]),
      }
    );
    const program = new Program<CounterIdl>(idl, provider);
    const controller = new AbortController();

    const received: bigint[] = [];
    const errors: [unknown, { fatal: boolean }][] = [];
    program.addEventListener(
      "incremented",
      (event) => received.push(event.count),
      {
        abortSignal: controller.signal,
        onError: (error, context) => errors.push([error, context]),
      }
    );
    await nextTick();
    controller.abort();

    expect(received).toEqual([3n]);
    expect(errors).toHaveLength(1);
    expect(String(errors[0][0])).toMatch(/Unexpected first log line/);
    expect(errors[0][1]).toEqual({ fatal: false });
  });

  it("stops listening when the signal is aborted", async () => {
    const aborted: AbortSignal[] = [];
    const { provider } = mockProvider(
      {},
      { subscriptions: logsSubscriptions([], [], aborted) }
    );
    const program = new Program<CounterIdl>(idl, provider);

    const controller = new AbortController();
    program.addEventListener("reset", () => {}, {
      abortSignal: controller.signal,
    });
    await nextTick();
    // The Kit subscription follows the caller's signal.
    expect(aborted).toHaveLength(1);
    expect(aborted[0].aborted).toBe(false);

    controller.abort();
    expect(aborted[0].aborted).toBe(true);
  });

  it("ends the listener when the error handler throws", async () => {
    const aborted: AbortSignal[] = [];
    const { provider } = mockProvider(
      {},
      {
        subscriptions: logsSubscriptions(
          [
            logsNotification([
              eventLog([1, 2, 3, 4, 5, 6, 7, 8], 1n),
              eventLog([1, 2, 3, 4, 5, 6, 7, 8], 2n),
            ]),
            logsNotification([eventLog([1, 2, 3, 4, 5, 6, 7, 8], 3n)]),
          ],
          [],
          aborted
        ),
      }
    );
    const program = new Program<CounterIdl>(idl, provider);
    const callbackFailure = new Error("callback failed");
    const handlerFailure = new Error("handler failed");

    const received: bigint[] = [];
    const errors: [unknown, { fatal: boolean }][] = [];
    program.addEventListener(
      "incremented",
      (event) => {
        received.push(event.count);
        if (event.count === 1n) throw callbackFailure;
      },
      {
        abortSignal: new AbortController().signal,
        onError: (error, context) => {
          errors.push([error, context]);
          throw handlerFailure;
        },
      }
    );
    await nextTick();

    // The handler broke its contract on the first report: it is told once,
    // as a fatal failure, and nothing else is delivered or reported.
    expect(errors).toEqual([
      [callbackFailure, { fatal: false }],
      [handlerFailure, { fatal: true }],
    ]);
    expect(received).toEqual([1n]);
    expect(aborted[0].aborted).toBe(true);
  });

  it("ends the listener silently on subscription failure without a handler", async () => {
    const { provider } = mockProvider(
      {},
      {
        subscriptions: {
          logsNotifications: () => ({
            subscribe: async () => {
              throw new Error("websocket closed");
            },
          }),
        },
      }
    );
    const program = new Program<CounterIdl>(idl, provider);

    program.addEventListener("reset", () => {}, {
      abortSignal: new AbortController().signal,
    });
    // Nothing to assert beyond the absence of an unhandled rejection, which
    // would fail the test run.
    await nextTick();
  });

  it("forwards the listener commitment and reports subscription failures", async () => {
    const calls: unknown[][] = [];
    const failure = new Error("websocket closed");
    const { provider } = mockProvider(
      {},
      {
        subscriptions: {
          logsNotifications: (...args: unknown[]) => {
            calls.push(args);
            return {
              subscribe: async () => {
                throw failure;
              },
            };
          },
        },
      }
    );
    const program = new Program<CounterIdl>(idl, provider);

    const reported = await new Promise<[unknown, { fatal: boolean }]>(
      (resolve) => {
        program.addEventListener("reset", () => {}, {
          abortSignal: new AbortController().signal,
          commitment: "processed",
          onError: (error, context) => resolve([error, context]),
        });
      }
    );

    // The subscription itself failed: the listener is finished.
    expect(reported).toEqual([failure, { fatal: true }]);
    expect(calls[0][1]).toEqual({ commitment: "processed" });
  });

  it("requires the provider to have subscriptions", () => {
    const { provider: inner } = mockProvider({});
    const program = new Program<CounterIdl>(idl, { rpc: inner.rpc });

    expect(() =>
      program.addEventListener("reset", () => {}, {
        abortSignal: new AbortController().signal,
      })
    ).toThrow("`rpcSubscriptions`");
  });
});
