import { Commitment, PublicKey } from "@solana/web3.js";
import { Coder } from "../coder/index.js";
import { IdlEvent, IdlField } from "../idl.js";
import Provider from "../provider.js";
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

type EventCallback = (event: any, slot: number, signature: string) => void;

export class EventManager {
  /**
   * Program ID for event subscriptions.
   */
  private _programId: PublicKey;

  /**
   * Network and wallet provider.
   */
  private _provider: Provider;

  /**
   * Event parser to handle onLogs callbacks.
   */
  private _eventParser: EventParser;

  /**
   * Maps event listener id to [event-name, callback].
   */
  private _eventCallbacks: Map<number, [string, EventCallback]>;

  /**
   * Maps event name to all listeners for the event.
   */
  private _eventListeners: Map<string, Array<number>>;

  /**
   * The next listener id to allocate.
   */
  private _listenerIdCount: number;

  /**
   * The subscription id from the connection onLogs subscription.
   */
  private _onLogsSubscriptionId: number | undefined;

  constructor(programId: PublicKey, provider: Provider, coder: Coder) {
    this._programId = programId;
    this._provider = provider;
    this._eventParser = new EventParser(programId, coder);
    this._eventCallbacks = new Map();
    this._eventListeners = new Map();
    this._listenerIdCount = 0;
  }

  public addEventListener(
    eventName: string,
    callback: (event: any, slot: number, signature: string) => void,
    commitment?: Commitment
  ): number {
    let listener = this._listenerIdCount;
    this._listenerIdCount += 1;

    // Store the listener into the event map.
    if (!this._eventListeners.has(eventName)) {
      this._eventListeners.set(eventName, []);
    }
    this._eventListeners.set(
      eventName,
      (this._eventListeners.get(eventName) ?? []).concat(listener)
    );

    // Store the callback into the listener map.
    this._eventCallbacks.set(listener, [eventName, callback]);

    // Create the subscription singleton, if needed.
    if (this._onLogsSubscriptionId !== undefined) {
      return listener;
    }

    this._onLogsSubscriptionId = this._provider!.connection.onLogs(
      this._programId,
      (logs, ctx) => {
        if (logs.err) {
          return;
        }

        for (const event of this._eventParser.parseLogs(logs.logs)) {
          const allListeners = this._eventListeners.get(event.name);

          if (allListeners) {
            allListeners.forEach((listener) => {
              const listenerCb = this._eventCallbacks.get(listener);

              if (listenerCb) {
                const [, callback] = listenerCb;
                callback(event.data, ctx.slot, logs.signature);
              }
            });
          }
        }
      },
      commitment
    );

    return listener;
  }

  public async removeEventListener(listener: number): Promise<void> {
    // Get the callback.
    const callback = this._eventCallbacks.get(listener);
    if (!callback) {
      throw new Error(`Event listener ${listener} doesn't exist!`);
    }
    const [eventName] = callback;

    // Get the listeners.
    let listeners = this._eventListeners.get(eventName);
    if (!listeners) {
      throw new Error(`Event listeners don't exist for ${eventName}!`);
    }

    // Update both maps.
    this._eventCallbacks.delete(listener);
    listeners = listeners.filter((l) => l !== listener);
    this._eventListeners.set(eventName, listeners);
    if (listeners.length === 0) {
      this._eventListeners.delete(eventName);
    }

    // Kill the websocket connection if all listeners have been removed.
    if (this._eventCallbacks.size === 0) {
      if (this._eventListeners.size !== 0) {
        throw new Error(
          `Expected event listeners size to be 0 but got ${this._eventListeners.size}`
        );
      }

      if (this._onLogsSubscriptionId !== undefined) {
        await this._provider!.connection.removeOnLogsListener(
          this._onLogsSubscriptionId
        );
        this._onLogsSubscriptionId = undefined;
      }
    }
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
    logs: string[],
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
    const invokeMatch = EventParser.INVOKE_RE.exec(log);
    if (invokeMatch && invokeMatch[1] === this.programId.toString()) {
      return [this.programId.toString(), false];
    } else if (log.startsWith(`Program ${this.programId.toString()} log: `)) {
      return [this.programId.toString(), false];
    } else if (invokeMatch && invokeMatch[2] !== EventParser.ROOT_DEPTH) {
      return ["cpi", false];
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
