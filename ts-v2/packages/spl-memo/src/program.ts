import { PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider } from "@anchor-lang/core";

import { SplMemoCoder } from "./coder";

export const SPL_MEMO_PROGRAM_ID = new PublicKey(
  "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo"
);

interface GetProgramParams {
  programId?: PublicKey;
  provider?: AnchorProvider;
}

export function splMemoProgram(params?: GetProgramParams): Program<SplMemo> {
  return new Program<SplMemo>(
    IDL,
    params?.programId ?? SPL_MEMO_PROGRAM_ID,
    params?.provider,
    new SplMemoCoder(IDL)
  );
}

type SplMemo = {
  version: "3.0.1";
  name: "spl_memo";
  instructions: [
    {
      name: "addMemo";
      accounts: [];
      args: [
        {
          name: "memo";
          type: "string";
        }
      ];
    }
  ];
};

const IDL: SplMemo = {
  version: "3.0.1",
  name: "spl_memo",
  instructions: [
    {
      name: "addMemo",
      accounts: [],
      args: [
        {
          name: "memo",
          type: "string",
        },
      ],
    },
  ],
};
