import { Idl, TypesCoder } from "@anchor-lang/core";

export class SplTokenTypesCoder implements TypesCoder {
  constructor(_idl: Idl) {}

  encode<T = any>(_name: string, _type: T): Buffer {
    throw new Error("SplToken does not have user-defined types");
  }
  decode<T = any>(_name: string, _typeData: Buffer): T {
    throw new Error("SplToken does not have user-defined types");
  }
}
