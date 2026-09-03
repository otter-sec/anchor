import { PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider } from "@anchor-lang/core";

import { SplNameServiceCoder } from "./coder";

export const SPL_NAME_SERVICE_PROGRAM_ID = new PublicKey(
  "namesLPneVptA9Z5rqUDD9tMTWEJwofgaYwp8cawRkX"
);

interface GetProgramParams {
  programId?: PublicKey;
  provider?: AnchorProvider;
}

export function splNameServiceProgram(
  params?: GetProgramParams
): Program<SplNameService> {
  return new Program<SplNameService>(
    IDL,
    params?.programId ?? SPL_NAME_SERVICE_PROGRAM_ID,
    params?.provider,
    new SplNameServiceCoder(IDL)
  );
}

type SplNameService = {
  version: "0.2.0";
  name: "spl_name_service";
  instructions: [
    {
      name: "create";
      accounts: [
        {
          name: "systemProgram";
          isMut: false;
          isSigner: false;
        },
        {
          name: "payer";
          isMut: true;
          isSigner: true;
        },
        {
          name: "nameAccount";
          isMut: true;
          isSigner: false;
        },
        {
          name: "nameOwner";
          isMut: false;
          isSigner: false;
        }
      ];
      args: [
        {
          name: "hashedName";
          type: "bytes";
        },
        {
          name: "lamports";
          type: "u64";
        },
        {
          name: "space";
          type: "u32";
        }
      ];
    },
    {
      name: "update";
      accounts: [
        {
          name: "nameAccount";
          isMut: true;
          isSigner: false;
        },
        {
          name: "nameUpdateSigner";
          isMut: false;
          isSigner: true;
        }
      ];
      args: [
        {
          name: "offset";
          type: "u32";
        },
        {
          name: "data";
          type: "bytes";
        }
      ];
    },
    {
      name: "transfer";
      accounts: [
        {
          name: "nameAccount";
          isMut: true;
          isSigner: false;
        },
        {
          name: "nameOwner";
          isMut: false;
          isSigner: true;
        }
      ];
      args: [
        {
          name: "newOwner";
          type: "publicKey";
        }
      ];
    },
    {
      name: "delete";
      accounts: [
        {
          name: "nameAccount";
          isMut: true;
          isSigner: false;
        },
        {
          name: "nameOwner";
          isMut: false;
          isSigner: true;
        },
        {
          name: "refundTarget";
          isMut: true;
          isSigner: false;
        }
      ];
      args: [];
    }
  ];
  accounts: [
    {
      name: "nameRecordHeader";
      type: {
        kind: "struct";
        fields: [
          {
            name: "parentName";
            type: "publicKey";
          },
          {
            name: "owner";
            type: "publicKey";
          },
          {
            name: "class";
            type: "publicKey";
          }
        ];
      };
    }
  ];
  errors: [
    {
      code: 0;
      name: "OutOfSpace";
      msg: "Out of space";
    }
  ];
};

const IDL: SplNameService = {
  version: "0.2.0",
  name: "spl_name_service",
  instructions: [
    {
      name: "create",
      accounts: [
        {
          name: "systemProgram",
          isMut: false,
          isSigner: false,
        },
        {
          name: "payer",
          isMut: true,
          isSigner: true,
        },
        {
          name: "nameAccount",
          isMut: true,
          isSigner: false,
        },
        {
          name: "nameOwner",
          isMut: false,
          isSigner: false,
        },
      ],
      args: [
        {
          name: "hashedName",
          type: "bytes",
        },
        {
          name: "lamports",
          type: "u64",
        },
        {
          name: "space",
          type: "u32",
        },
      ],
    },
    {
      name: "update",
      accounts: [
        {
          name: "nameAccount",
          isMut: true,
          isSigner: false,
        },
        {
          name: "nameUpdateSigner",
          isMut: false,
          isSigner: true,
        },
      ],
      args: [
        {
          name: "offset",
          type: "u32",
        },
        {
          name: "data",
          type: "bytes",
        },
      ],
    },
    {
      name: "transfer",
      accounts: [
        {
          name: "nameAccount",
          isMut: true,
          isSigner: false,
        },
        {
          name: "nameOwner",
          isMut: false,
          isSigner: true,
        },
      ],
      args: [
        {
          name: "newOwner",
          type: "publicKey",
        },
      ],
    },
    {
      name: "delete",
      accounts: [
        {
          name: "nameAccount",
          isMut: true,
          isSigner: false,
        },
        {
          name: "nameOwner",
          isMut: false,
          isSigner: true,
        },
        {
          name: "refundTarget",
          isMut: true,
          isSigner: false,
        },
      ],
      args: [],
    },
  ],
  accounts: [
    {
      name: "nameRecordHeader",
      type: {
        kind: "struct",
        fields: [
          {
            name: "parentName",
            type: "publicKey",
          },
          {
            name: "owner",
            type: "publicKey",
          },
          {
            name: "class",
            type: "publicKey",
          },
        ],
      },
    },
  ],
  errors: [
    {
      code: 0,
      name: "OutOfSpace",
      msg: "Out of space",
    },
  ],
};
