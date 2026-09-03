import { PublicKey } from "@solana/web3.js";
import { Program } from "../program/index.js";
import Provider from "../provider.js";
import { SystemCoder } from "../coder/system/index.js";

export const SYSTEM_PROGRAM_ID = new PublicKey(
  "11111111111111111111111111111111"
);

export function program(provider?: Provider): Program<SystemProgram> {
  return new Program<SystemProgram>(IDL, provider, coder());
}

export function coder(): SystemCoder {
  return new SystemCoder(IDL);
}

/**
 * System IDL.
 */
export type SystemProgram = {
  address: "11111111111111111111111111111111";
  metadata: {
    name: "systemProgram";
    version: "0.1.0";
    spec: "0.1.0";
  };
  instructions: [
    {
      name: "advanceNonceAccount";
      discriminator: [4, 0, 0, 0];
      accounts: [
        {
          name: "nonce";
          writable: true;
        },
        {
          name: "recentBlockhashes";
        },
        {
          name: "authorized";
          signer: true;
        }
      ];
      args: [
        {
          name: "authorized";
          type: "pubkey";
        }
      ];
    },
    {
      name: "allocate";
      discriminator: [8, 0, 0, 0];
      accounts: [
        {
          name: "pubkey";
          writable: true;
          signer: true;
        }
      ];
      args: [
        {
          name: "space";
          type: "u64";
        }
      ];
    },
    {
      name: "allocateWithSeed";
      discriminator: [9, 0, 0, 0];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "base";
          signer: true;
        }
      ];
      args: [
        {
          name: "base";
          type: "pubkey";
        },
        {
          name: "seed";
          type: "string";
        },
        {
          name: "space";
          type: "u64";
        },
        {
          name: "owner";
          type: "pubkey";
        }
      ];
    },
    {
      name: "assign";
      discriminator: [1, 0, 0, 0];
      accounts: [
        {
          name: "pubkey";
          writable: true;
          signer: true;
        }
      ];
      args: [
        {
          name: "owner";
          type: "pubkey";
        }
      ];
    },
    {
      name: "assignWithSeed";
      discriminator: [10, 0, 0, 0];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "base";
          signer: true;
        }
      ];
      args: [
        {
          name: "base";
          type: "pubkey";
        },
        {
          name: "seed";
          type: "string";
        },
        {
          name: "owner";
          type: "pubkey";
        }
      ];
    },
    {
      name: "authorizeNonceAccount";
      discriminator: [7, 0, 0, 0];
      accounts: [
        {
          name: "nonce";
          writable: true;
        },
        {
          name: "authorized";
          signer: true;
        }
      ];
      args: [
        {
          name: "authorized";
          type: "pubkey";
        }
      ];
    },
    {
      name: "createAccount";
      discriminator: [0, 0, 0, 0];
      accounts: [
        {
          name: "from";
          writable: true;
          signer: true;
        },
        {
          name: "to";
          writable: true;
          signer: true;
        }
      ];
      args: [
        {
          name: "lamports";
          type: "u64";
        },
        {
          name: "space";
          type: "u64";
        },
        {
          name: "owner";
          type: "pubkey";
        }
      ];
    },
    {
      name: "createAccountWithSeed";
      discriminator: [3, 0, 0, 0];
      accounts: [
        {
          name: "from";
          writable: true;
          signer: true;
        },
        {
          name: "to";
          writable: true;
        },
        {
          name: "base";
          signer: true;
        }
      ];
      args: [
        {
          name: "base";
          type: "pubkey";
        },
        {
          name: "seed";
          type: "string";
        },
        {
          name: "lamports";
          type: "u64";
        },
        {
          name: "space";
          type: "u64";
        },
        {
          name: "owner";
          type: "pubkey";
        }
      ];
    },
    {
      name: "initializeNonceAccount";
      discriminator: [6, 0, 0, 0];
      accounts: [
        {
          name: "nonce";
          writable: true;
          signer: true;
        },
        {
          name: "recentBlockhashes";
        },
        {
          name: "rent";
          address: "SysvarRent111111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "authorized";
          type: "pubkey";
        }
      ];
    },
    {
      name: "transfer";
      discriminator: [2, 0, 0, 0];
      accounts: [
        {
          name: "from";
          writable: true;
          signer: true;
        },
        {
          name: "to";
          writable: true;
        }
      ];
      args: [
        {
          name: "lamports";
          type: "u64";
        }
      ];
    },
    {
      name: "transferWithSeed";
      discriminator: [11, 0, 0, 0];
      accounts: [
        {
          name: "from";
          writable: true;
        },
        {
          name: "base";
          signer: true;
        },
        {
          name: "to";
          writable: true;
        }
      ];
      args: [
        {
          name: "lamports";
          type: "u64";
        },
        {
          name: "seed";
          type: "string";
        },
        {
          name: "owner";
          type: "pubkey";
        }
      ];
    },
    {
      name: "withdrawNonceAccount";
      discriminator: [5, 0, 0, 0];
      accounts: [
        {
          name: "nonce";
          writable: true;
        },
        {
          name: "to";
          writable: true;
        },
        {
          name: "recentBlockhashes";
        },
        {
          name: "rent";
          address: "SysvarRent111111111111111111111111111111111";
        },
        {
          name: "authorized";
          signer: true;
        }
      ];
      args: [
        {
          name: "lamports";
          type: "u64";
        }
      ];
    }
  ];
  accounts: [
    {
      name: "nonce";
      discriminator: [];
    }
  ];
  types: [
    {
      name: "feeCalculator";
      type: {
        kind: "struct";
        fields: [
          {
            name: "lamportsPerSignature";
            type: "u64";
          }
        ];
      };
    },
    {
      name: "nonce";
      type: {
        kind: "struct";
        fields: [
          {
            name: "version";
            type: "u32";
          },
          {
            name: "state";
            type: "u32";
          },
          {
            name: "authorizedPubkey";
            type: "pubkey";
          },
          {
            name: "nonce";
            type: "pubkey";
          },
          {
            name: "feeCalculator";
            type: {
              defined: {
                name: "feeCalculator";
              };
            };
          }
        ];
      };
    }
  ];
};

export const IDL: SystemProgram = {
  address: "11111111111111111111111111111111",
  metadata: {
    name: "systemProgram",
    version: "0.1.0",
    spec: "0.1.0",
  },
  instructions: [
    {
      name: "advanceNonceAccount",
      discriminator: [4, 0, 0, 0],
      accounts: [
        {
          name: "nonce",
          writable: true,
        },
        {
          name: "recentBlockhashes",
        },
        {
          name: "authorized",
          signer: true,
        },
      ],
      args: [
        {
          name: "authorized",
          type: "pubkey",
        },
      ],
    },
    {
      name: "allocate",
      discriminator: [8, 0, 0, 0],
      accounts: [
        {
          name: "pubkey",
          writable: true,
          signer: true,
        },
      ],
      args: [
        {
          name: "space",
          type: "u64",
        },
      ],
    },
    {
      name: "allocateWithSeed",
      discriminator: [9, 0, 0, 0],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "base",
          signer: true,
        },
      ],
      args: [
        {
          name: "base",
          type: "pubkey",
        },
        {
          name: "seed",
          type: "string",
        },
        {
          name: "space",
          type: "u64",
        },
        {
          name: "owner",
          type: "pubkey",
        },
      ],
    },
    {
      name: "assign",
      discriminator: [1, 0, 0, 0],
      accounts: [
        {
          name: "pubkey",
          writable: true,
          signer: true,
        },
      ],
      args: [
        {
          name: "owner",
          type: "pubkey",
        },
      ],
    },
    {
      name: "assignWithSeed",
      discriminator: [10, 0, 0, 0],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "base",
          signer: true,
        },
      ],
      args: [
        {
          name: "base",
          type: "pubkey",
        },
        {
          name: "seed",
          type: "string",
        },
        {
          name: "owner",
          type: "pubkey",
        },
      ],
    },
    {
      name: "authorizeNonceAccount",
      discriminator: [7, 0, 0, 0],
      accounts: [
        {
          name: "nonce",
          writable: true,
        },
        {
          name: "authorized",
          signer: true,
        },
      ],
      args: [
        {
          name: "authorized",
          type: "pubkey",
        },
      ],
    },
    {
      name: "createAccount",
      discriminator: [0, 0, 0, 0],
      accounts: [
        {
          name: "from",
          writable: true,
          signer: true,
        },
        {
          name: "to",
          writable: true,
          signer: true,
        },
      ],
      args: [
        {
          name: "lamports",
          type: "u64",
        },
        {
          name: "space",
          type: "u64",
        },
        {
          name: "owner",
          type: "pubkey",
        },
      ],
    },
    {
      name: "createAccountWithSeed",
      discriminator: [3, 0, 0, 0],
      accounts: [
        {
          name: "from",
          writable: true,
          signer: true,
        },
        {
          name: "to",
          writable: true,
        },
        {
          name: "base",
          signer: true,
        },
      ],
      args: [
        {
          name: "base",
          type: "pubkey",
        },
        {
          name: "seed",
          type: "string",
        },
        {
          name: "lamports",
          type: "u64",
        },
        {
          name: "space",
          type: "u64",
        },
        {
          name: "owner",
          type: "pubkey",
        },
      ],
    },
    {
      name: "initializeNonceAccount",
      discriminator: [6, 0, 0, 0],
      accounts: [
        {
          name: "nonce",
          writable: true,
          signer: true,
        },
        {
          name: "recentBlockhashes",
        },
        {
          name: "rent",
          address: "SysvarRent111111111111111111111111111111111",
        },
      ],
      args: [
        {
          name: "authorized",
          type: "pubkey",
        },
      ],
    },
    {
      name: "transfer",
      discriminator: [2, 0, 0, 0],
      accounts: [
        {
          name: "from",
          writable: true,
          signer: true,
        },
        {
          name: "to",
          writable: true,
        },
      ],
      args: [
        {
          name: "lamports",
          type: "u64",
        },
      ],
    },
    {
      name: "transferWithSeed",
      discriminator: [11, 0, 0, 0],
      accounts: [
        {
          name: "from",
          writable: true,
        },
        {
          name: "base",
          signer: true,
        },
        {
          name: "to",
          writable: true,
        },
      ],
      args: [
        {
          name: "lamports",
          type: "u64",
        },
        {
          name: "seed",
          type: "string",
        },
        {
          name: "owner",
          type: "pubkey",
        },
      ],
    },
    {
      name: "withdrawNonceAccount",
      discriminator: [5, 0, 0, 0],
      accounts: [
        {
          name: "nonce",
          writable: true,
        },
        {
          name: "to",
          writable: true,
        },
        {
          name: "recentBlockhashes",
        },
        {
          name: "rent",
          address: "SysvarRent111111111111111111111111111111111",
        },
        {
          name: "authorized",
          signer: true,
        },
      ],
      args: [
        {
          name: "lamports",
          type: "u64",
        },
      ],
    },
  ],
  accounts: [
    {
      name: "nonce",
      discriminator: [],
    },
  ],
  types: [
    {
      name: "feeCalculator",
      type: {
        kind: "struct",
        fields: [
          {
            name: "lamportsPerSignature",
            type: "u64",
          },
        ],
      },
    },
    {
      name: "nonce",
      type: {
        kind: "struct",
        fields: [
          {
            name: "version",
            type: "u32",
          },
          {
            name: "state",
            type: "u32",
          },
          {
            name: "authorizedPubkey",
            type: "pubkey",
          },
          {
            name: "nonce",
            type: "pubkey",
          },
          {
            name: "feeCalculator",
            type: {
              defined: {
                name: "feeCalculator",
              },
            },
          },
        ],
      },
    },
  ],
};
