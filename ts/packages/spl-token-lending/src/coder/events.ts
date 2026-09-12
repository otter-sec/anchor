import { Idl, Event, EventCoder } from "@anchor-lang/core";
import { IdlEvent } from "@anchor-lang/core/dist/cjs/idl";

export class SplTokenLendingEventsCoder implements EventCoder {
  constructor(_idl: Idl) {}

  decode<E extends IdlEvent = IdlEvent, T = Record<string, string>>(
    _log: string
  ): Event<E, T> | null {
    throw new Error("SplTokenLending program does not have events");
  }
}
