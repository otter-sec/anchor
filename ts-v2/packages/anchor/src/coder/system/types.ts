import { TypesCoder } from "../index.js";
import { Idl } from "../../idl.js";

export class SystemTypesCoder implements TypesCoder {
  constructor(_idl: Idl) {}

  encode<T = any>(_name: string, _type: T): Buffer {
    throw new Error("System does not have user-defined types");
  }
  decode<T = any>(_name: string, _typeData: Buffer): T {
    throw new Error("System does not have user-defined types");
  }
}
