import { Idl, TypesCoder } from "@anchor-lang/core";

export class SplRecordTypesCoder implements TypesCoder {
  constructor(_idl: Idl) {}

  encode<T = any>(_name: string, _type: T): Buffer {
    throw new Error("SplRecord does not have user-defined types");
  }
  decode<T = any>(_name: string, _typeData: Buffer): T {
    throw new Error("SplRecord does not have user-defined types");
  }
}
