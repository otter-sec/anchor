import { PublicKey } from "@solana/web3.js";
import { Coder } from "../../coder/index.js";
import Provider from "../../provider.js";
import { Idl, IdlInstruction } from "../../idl.js";
import InstructionFactory, { InstructionNamespace } from "./instruction.js";
import TransactionFactory, { TransactionNamespace } from "./transaction.js";
import RpcFactory, { RpcNamespace } from "./rpc.js";
import AccountFactory, { AccountNamespace } from "./account.js";
import SimulateFactory, { SimulateNamespace } from "./simulate.js";
import { parseIdlErrors } from "../common.js";
import { MethodsBuilderFactory, MethodsNamespace } from "./methods";
import ViewFactory, { ViewNamespace } from "./views";
import { CustomAccountResolver } from "../accounts-resolver.js";

// Re-exports.
export { InstructionNamespace, InstructionFn } from "./instruction.js";
export { TransactionNamespace, TransactionFn } from "./transaction.js";
export { RpcNamespace, RpcFn } from "./rpc.js";
export { AccountNamespace, AccountClient, ProgramAccount } from "./account.js";
export { SimulateNamespace, SimulateFn } from "./simulate.js";
export { IdlAccounts, IdlTypes, DecodeType, IdlEvents } from "./types.js";
export { MethodsBuilderFactory, MethodsNamespace } from "./methods";
export { ViewNamespace, ViewFn } from "./views";

export default class NamespaceFactory {
  /**
   * Generates all namespaces for a given program.
   */
  public static build<IDL extends Idl>(
    idl: IDL,
    coder: Coder,
    programId: PublicKey,
    provider: Provider,
    getCustomResolver?: (
      instruction: IdlInstruction
    ) => CustomAccountResolver<IDL> | undefined
  ): [
    RpcNamespace<IDL>,
    InstructionNamespace<IDL>,
    TransactionNamespace<IDL>,
    AccountNamespace<IDL>,
    SimulateNamespace<IDL>,
    MethodsNamespace<IDL>,
    ViewNamespace<IDL> | undefined
  ] {
    const rpc: RpcNamespace = {};
    const instruction: InstructionNamespace = {};
    const transaction: TransactionNamespace = {};
    const simulate: SimulateNamespace = {};
    const methods: MethodsNamespace = {};
    const view: ViewNamespace = {};

    const idlErrors = parseIdlErrors(idl);

    const account: AccountNamespace<IDL> = idl.accounts
      ? AccountFactory.build(idl, coder, programId, provider)
      : ({} as AccountNamespace<IDL>);

    idl.instructions.forEach((idlIx) => {
      const ixItem = InstructionFactory.build<IDL, typeof idlIx>(
        idlIx,
        (ixName, ix) => coder.instruction.encode(ixName, ix),
        programId
      );
      const txItem = TransactionFactory.build(idlIx, ixItem);
      const rpcItem = RpcFactory.build(idlIx, txItem, idlErrors, provider);
      const simulateItem = SimulateFactory.build(
        idlIx,
        txItem,
        idlErrors,
        provider,
        coder,
        programId,
        idl
      );
      const viewItem = ViewFactory.build(programId, idlIx, simulateItem, idl);
      const methodItem = MethodsBuilderFactory.build<IDL, typeof idlIx>(
        provider,
        programId,
        idlIx,
        ixItem,
        txItem,
        rpcItem,
        simulateItem,
        viewItem,
        account,
        idl.types || [],
        getCustomResolver?.(idlIx)
      );
      const name = idlIx.name;

      instruction[name] = ixItem;
      transaction[name] = txItem;
      rpc[name] = rpcItem;
      simulate[name] = simulateItem;
      methods[name] = methodItem;
      if (viewItem) {
        view[name] = viewItem;
      }
    });

    return [
      rpc as RpcNamespace<IDL>,
      instruction as InstructionNamespace<IDL>,
      transaction as TransactionNamespace<IDL>,
      account,
      simulate as SimulateNamespace<IDL>,
      methods as MethodsNamespace<IDL>,
      view as ViewNamespace<IDL>,
    ];
  }
}
