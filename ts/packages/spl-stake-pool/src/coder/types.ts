import { Idl, TypesCoder } from "@anchor-lang/core";

export class SplStakePoolTypesCoder implements TypesCoder {
  constructor(_idl: Idl) {}

  encode<T = any>(_name: string, _type: T): Buffer {
    throw new Error("SplStakePool does not have user-defined types");
  }
  decode<T = any>(_name: string, _typeData: Buffer): T {
    throw new Error("SplStakePool does not have user-defined types");
  }
}
