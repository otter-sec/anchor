import { Program, Provider } from "../index.js";
import { program as systemProgram, SystemProgram } from "./system.js";

export { SystemProgram } from "./system.js";

export class Native {
  public static system(provider?: Provider): Program<SystemProgram> {
    return systemProgram(provider);
  }
}
