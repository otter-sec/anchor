import { Address, Commitment, Signature, Slot } from "@solana/kit";
import { PublicKey } from "@solana/web3.js";
import { Coder } from "../coder/index.js";
import { IdlEvent, IdlField } from "../idl.js";
import Provider from "../provider.js";
import { withProviderDefaults } from "../utils/common.js";
import { toAddress } from "./common.js";
import { DecodeType } from "./namespace/types.js";

const PROGRAM_LOG = "Program log: ";
const PROGRAM_DATA = "Program data: ";
const PROGRAM_LOG_START_INDEX = PROGRAM_LOG.length;
const PROGRAM_DATA_START_INDEX = PROGRAM_DATA.length;

// Deserialized event.
export type Event<
  E extends IdlEvent = IdlEvent,
  Defined = Record<string, never>
> = {
  name: E["name"];
  // TODO:
  // data: EventData<E["type"]["fields"][number], Defined>;
  data: any;
};

export type EventData<T extends IdlField, Defined> = {
  [N in T["name"]]: DecodeType<(T & { name: N })["type"], Defined>;
};

/**
 * Options of {@link EventManager.addEventListener}.
 */
export type EventListenerOptions = {
  /** Stops listening when aborted. */
  abortSignal: AbortSignal;
  /** The commitment to listen at, defaulting to the provider's. */
  commitment?: Commitment;
  /**
   * Invoked when a notification cannot be processed (unparseable logs, a
   * failing decode, or the callback throwing for one event), in which case
   * the listener keeps going, and when the subscription itself fails, in
   * which case `fatal` is true and no further events are delivered: listen
   * again with a fresh abort signal to resubscribe. Must not throw: a
   * throwing handler ends the listener, its error reported once as fatal.
   * When omitted, failures are dropped and a failing subscription ends the
   * listener silently.
   */
  onError?: (error: unknown, context: { fatal: boolean }) => void;
};

export class EventManager {
  /**
   * Program address for event subscriptions.
   */
  private _programAddress: Address;

  /**
   * Network and wallet provider.
   */
  private _provider: Provider;

  /**
   * Event parser to handle log notifications.
   */
  private _eventParser: EventParser;

  constructor(programId: PublicKey, provider: Provider, coder: Coder) {
    this._programAddress = toAddress(programId);
    this._provider = provider;
    this._eventParser = new EventParser(programId, coder);
  }

  /**
   * Invokes the callback for every emission of the given event, listening to
   * the program's logs at the given commitment (the provider's by default)
   * until the abort signal fires.
   *
   * Each listener holds its own log subscription; Kit coalesces identical
   * subscriptions into a single one on the wire. A notification that cannot
   * be processed, or an event the callback throws on, is reported through
   * `onError` and skipped; only a failure of the subscription itself ends
   * the listener.
   */
  public addEventListener(
    eventName: string,
    callback: (event: any, slot: Slot, signature: Signature) => void,
    options: EventListenerOptions
  ): void {
    const { rpcSubscriptions } = this._provider;
    if (!rpcSubscriptions) {
      throw new Error(
        "Listening to events requires the provider to have an " +
          "`rpcSubscriptions` client."
      );
    }
    const { onError } = options;
    const { commitment } = withProviderDefaults(this._provider, {
      commitment: options.commitment,
    });

    // The subscription is bounded by a signal of our own so that the
    // listener can also be ended from the inside, when the error handler
    // breaks its contract.
    const controller = new AbortController();
    options.abortSignal.addEventListener("abort", () => controller.abort(), {
      once: true,
    });
    if (options.abortSignal.aborted) {
      controller.abort();
    }
    const abortSignal = controller.signal;

    // Hands a failure to `onError`. A handler that throws ends the listener:
    // its own error is reported once as fatal, so that a misbehaving handler
    // never surfaces as an unhandled rejection. Nothing is reported once the
    // listener has ended.
    const report = (error: unknown, fatal: boolean) => {
      if (abortSignal.aborted) {
        return;
      }
      try {
        onError?.(error, { fatal });
      } catch (handlerError) {
        controller.abort();
        try {
          onError?.(handlerError, { fatal: true });
        } catch {
          // The handler threw while being told that it threw: there is no
          // one left to report to.
        }
      }
    };

    (async () => {
      const notifications = await rpcSubscriptions
        .logsNotifications(
          { mentions: [this._programAddress] },
          commitment ? { commitment } : {}
        )
        .subscribe({ abortSignal });
      for await (const { context, value } of notifications) {
        if (value.err) {
          continue;
        }
        try {
          for (const event of this._eventParser.parseLogs(value.logs)) {
            if (abortSignal.aborted) {
              return;
            }
            if (event.name !== eventName) {
              continue;
            }
            try {
              callback(event.data, context.slot, value.signature);
            } catch (error) {
              report(error, false);
            }
          }
        } catch (error) {
          report(error, false);
        }
      }
    })().catch((error) => report(error, true));
  }
}

export class EventParser {
  private coder: Coder;
  private programId: PublicKey;
  private static readonly INVOKE_RE =
    /^Program ([1-9A-HJ-NP-Za-km-z]+) invoke \[(\d+)\]$/;
  private static readonly ROOT_DEPTH = "1";

  constructor(programId: PublicKey, coder: Coder) {
    this.coder = coder;
    this.programId = programId;
  }

  // Each log given, represents an array of messages emitted by
  // a single transaction, which can execute many different programs across
  // CPI boundaries. However, the subscription is only interested in the
  // events emitted by *this* program. In achieving this, we keep track of the
  // program execution context by parsing each log and looking for a CPI
  // `invoke` call. If one exists, we know a new program is executing. So we
  // push the programId onto a stack and switch the program context. This
  // allows us to track, for a given log, which program was executing during
  // its emission, thereby allowing us to know if a given log event was
  // emitted by *this* program. If it was, then we parse the raw string and
  // emit the event if the string matches the event being subscribed to.
  public *parseLogs(
    logs: readonly string[],
    errorOnDecodeFailure = false
  ): Generator<Event> {
    const scanner = new LogScanner([...logs]);
    const execution = new ExecutionContext();

    const firstLog = scanner.next();
    if (firstLog === null) return;

    const firstCap = EventParser.INVOKE_RE.exec(firstLog);
    if (!firstCap || firstCap[2] !== EventParser.ROOT_DEPTH) {
      throw new Error(`Unexpected first log line: ${firstLog}`);
    }
    execution.push(firstCap[1]);

    while (scanner.peek() !== null) {
      const log = scanner.next();
      if (log === null) break;

      let [event, newProgram, didPop] = this.handleLog(
        execution,
        log,
        errorOnDecodeFailure
      );

      if (event) yield event;
      if (newProgram) execution.push(newProgram);

      if (didPop) {
        execution.pop();
        const nextLog = scanner.peek();
        if (nextLog && nextLog.endsWith("invoke [1]")) {
          const m = EventParser.INVOKE_RE.exec(nextLog);
          if (m) execution.push(m[1]);
        }
      }
    }
  }

  // Main log handler. Returns a three element array of the event, the
  // next program that was invoked for CPI, and a boolean indicating if
  // a program has completed execution (and thus should be popped off the
  // execution stack).
  private handleLog(
    execution: ExecutionContext,
    log: string,
    errorOnDecodeFailure: boolean
  ): [Event | null, string | null, boolean] {
    // Executing program is this program.
    if (
      execution.stack.length > 0 &&
      execution.program() === this.programId.toString()
    ) {
      return this.handleProgramLog(log, errorOnDecodeFailure);
    }
    // Executing program is not this program.
    else {
      return [null, ...this.handleSystemLog(log)];
    }
  }

  // Handles logs from *this* program.
  private handleProgramLog(
    log: string,
    errorOnDecodeFailure: boolean
  ): [Event | null, string | null, boolean] {
    // This is a `msg!` log or a `sol_log_data` log.
    if (log.startsWith(PROGRAM_LOG) || log.startsWith(PROGRAM_DATA)) {
      const logStr = log.startsWith(PROGRAM_LOG)
        ? log.slice(PROGRAM_LOG_START_INDEX)
        : log.slice(PROGRAM_DATA_START_INDEX);
      const event = this.coder.events.decode(logStr);

      if (errorOnDecodeFailure && event === null) {
        throw new Error(`Unable to decode event ${logStr}`);
      }
      return [event, null, false];
    }
    // System log.
    else {
      return [null, ...this.handleSystemLog(log)];
    }
  }

  // Handles logs when the current program being executing is *not* this.
  private handleSystemLog(log: string): [string | null, boolean] {
    if (log.startsWith(`Program ${this.programId.toString()} log:`)) {
      return [this.programId.toString(), false];
    } else if (log.includes("invoke") && !log.endsWith("[1]")) {
      // Extract the invoked program ID from `Program <id> invoke [N]`
      // and push IT onto the execution stack — not a literal "cpi"
      // marker. If the CPI happens to land back in *this* program
      // (self-CPI), the next log will correctly report the invoked
      // program as equal to `this.programId` and get routed to
      // `handleProgramLog`, which can then decode events emitted
      // inside the CPI.
      const cpiMatch = EventParser.INVOKE_RE.exec(log);
      return [cpiMatch ? cpiMatch[1] : "cpi", false];
    } else {
      let regex = /^Program ([1-9A-HJ-NP-Za-km-z]+) success$/;
      if (regex.test(log)) {
        return [null, true];
      } else {
        return [null, false];
      }
    }
  }
}

// Stack frame execution context, allowing one to track what program is
// executing for a given log.
class ExecutionContext {
  stack: string[] = [];

  program(): string {
    if (!this.stack.length) {
      throw new Error("Expected the stack to have elements");
    }
    return this.stack[this.stack.length - 1];
  }

  push(newProgram: string) {
    this.stack.push(newProgram);
  }

  pop() {
    if (!this.stack.length) {
      throw new Error("Expected the stack to have elements");
    }
    this.stack.pop();
  }
}

class LogScanner {
  constructor(public logs: string[]) {
    // remove any logs that don't start with "Program "
    // this can happen in loader logs.
    // e.g. 3psYALQ9s7SjdezXw2kxKkVuQLtSAQxPAjETvy765EVxJE7cYqfc4oGbpNYEWsAiuXuTnqcsSUHLQ3iZUenTHTsA on devnet
    this.logs = this.logs.filter((log) => log.startsWith("Program "));
  }

  next(): string | null {
    if (this.logs.length === 0) {
      return null;
    }
    let l = this.logs[0];
    this.logs = this.logs.slice(1);
    return l;
  }

  peek(): string | null {
    return this.logs.length > 0 ? this.logs[0] : null;
  }
}
