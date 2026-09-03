import { PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider } from "@anchor-lang/core";

import { SplTokenCoder } from "./coder";

export const SPL_TOKEN_PROGRAM_ID = new PublicKey(
  "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
);

interface GetProgramParams {
  programId?: PublicKey;
  provider?: AnchorProvider;
}

export function splTokenProgram(params?: GetProgramParams): Program<SplToken> {
  return new Program<SplToken>(
    params?.programId ? { ...IDL, address: params.programId.toString() } : IDL,
    params?.provider,
    new SplTokenCoder(IDL)
  );
}

type SplToken = {
  address: string;
  metadata: {
    name: "splToken";
    version: "3.3.0";
    spec: "0.1.0";
  };
  instructions: [
    {
      name: "amountToUiAmount";
      discriminator: [23];
      accounts: [
        {
          name: "mint";
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        }
      ];
    },
    {
      name: "approve";
      discriminator: [4];
      accounts: [
        {
          name: "source";
          writable: true;
        },
        {
          name: "delegate";
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        }
      ];
    },
    {
      name: "approveChecked";
      discriminator: [13];
      accounts: [
        {
          name: "source";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "delegate";
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        },
        {
          name: "decimals";
          type: "u8";
        }
      ];
    },
    {
      name: "burn";
      discriminator: [8];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
          writable: true;
        },
        {
          name: "authority";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        }
      ];
    },
    {
      name: "burnChecked";
      discriminator: [15];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
          writable: true;
        },
        {
          name: "authority";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        },
        {
          name: "decimals";
          type: "u8";
        }
      ];
    },
    {
      name: "closeAccount";
      discriminator: [9];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "destination";
          writable: true;
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [];
    },
    {
      name: "freezeAccount";
      discriminator: [10];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [];
    },
    {
      name: "getAccountDataSize";
      discriminator: [21];
      accounts: [
        {
          name: "mint";
        }
      ];
      args: [];
    },
    {
      name: "initializeAccount";
      discriminator: [1];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "owner";
        },
        {
          name: "rent";
          address: "SysvarRent111111111111111111111111111111111";
        }
      ];
      args: [];
    },
    {
      name: "initializeAccount2";
      discriminator: [16];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "rent";
          address: "SysvarRent111111111111111111111111111111111";
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
      name: "initializeAccount3";
      discriminator: [18];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
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
      name: "initializeImmutableOwner";
      discriminator: [22];
      accounts: [
        {
          name: "account";
          writable: true;
        }
      ];
      args: [];
    },
    {
      name: "initializeMint";
      discriminator: [0];
      accounts: [
        {
          name: "mint";
          writable: true;
        },
        {
          name: "rent";
          address: "SysvarRent111111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "decimals";
          type: "u8";
        },
        {
          name: "mintAuthority";
          type: "pubkey";
        },
        {
          name: "freezeAuthority";
          type: {
            coption: "pubkey";
          };
        }
      ];
    },
    {
      name: "initializeMint2";
      discriminator: [20];
      accounts: [
        {
          name: "mint";
          writable: true;
        }
      ];
      args: [
        {
          name: "decimals";
          type: "u8";
        },
        {
          name: "mintAuthority";
          type: "pubkey";
        },
        {
          name: "freezeAuthority";
          type: {
            coption: "pubkey";
          };
        }
      ];
    },
    {
      name: "initializeMultisig";
      discriminator: [2];
      accounts: [
        {
          name: "multisig";
          writable: true;
        },
        {
          name: "rent";
          address: "SysvarRent111111111111111111111111111111111";
        }
      ];
      args: [
        {
          name: "m";
          type: "u8";
        }
      ];
    },
    {
      name: "initializeMultisig2";
      discriminator: [19];
      accounts: [
        {
          name: "multisig";
          writable: true;
        },
        {
          name: "signer";
        }
      ];
      args: [
        {
          name: "m";
          type: "u8";
        }
      ];
    },
    {
      name: "mintTo";
      discriminator: [7];
      accounts: [
        {
          name: "mint";
          writable: true;
        },
        {
          name: "account";
          writable: true;
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        }
      ];
    },
    {
      name: "mintToChecked";
      discriminator: [14];
      accounts: [
        {
          name: "mint";
          writable: true;
        },
        {
          name: "account";
          writable: true;
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        },
        {
          name: "decimals";
          type: "u8";
        }
      ];
    },
    {
      name: "revoke";
      discriminator: [5];
      accounts: [
        {
          name: "source";
          writable: true;
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [];
    },
    {
      name: "setAuthority";
      discriminator: [6];
      accounts: [
        {
          name: "owned";
          writable: true;
        },
        {
          name: "owner";
          signer: true;
        },
        {
          name: "signer";
          signer: true;
        }
      ];
      args: [
        {
          name: "authorityType";
          type: {
            defined: {
              name: "authorityType";
            };
          };
        },
        {
          name: "newAuthority";
          type: {
            coption: "pubkey";
          };
        }
      ];
    },
    {
      name: "syncNative";
      discriminator: [17];
      accounts: [
        {
          name: "account";
          writable: true;
        }
      ];
      args: [];
    },
    {
      name: "thawAccount";
      discriminator: [11];
      accounts: [
        {
          name: "account";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "owner";
          signer: true;
        }
      ];
      args: [];
    },
    {
      name: "transfer";
      discriminator: [3];
      accounts: [
        {
          name: "source";
          writable: true;
        },
        {
          name: "destination";
          writable: true;
        },
        {
          name: "authority";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        }
      ];
    },
    {
      name: "transferChecked";
      discriminator: [12];
      accounts: [
        {
          name: "source";
          writable: true;
        },
        {
          name: "mint";
        },
        {
          name: "destination";
          writable: true;
        },
        {
          name: "authority";
          signer: true;
        }
      ];
      args: [
        {
          name: "amount";
          type: "u64";
        },
        {
          name: "decimals";
          type: "u8";
        }
      ];
    },
    {
      name: "uiAmountToAmount";
      discriminator: [24];
      accounts: [
        {
          name: "mint";
        }
      ];
      args: [
        {
          name: "uiAmount";
          type: "string";
        }
      ];
    }
  ];
  accounts: [
    {
      name: "mint";
      discriminator: [];
    },
    {
      name: "multisig";
      discriminator: [];
    },
    {
      name: "account";
      discriminator: [];
    }
  ];
  errors: [
    {
      code: 0;
      name: "notRentExempt";
      msg: "Lamport balance below rent-exempt threshold";
    },
    {
      code: 1;
      name: "insufficientFunds";
      msg: "Insufficient funds";
    },
    {
      code: 2;
      name: "invalidMint";
      msg: "Invalid Mint";
    },
    {
      code: 3;
      name: "mintMismatch";
      msg: "Account not associated with this Mint";
    },
    {
      code: 4;
      name: "ownerMismatch";
      msg: "Owner does not match";
    },
    {
      code: 5;
      name: "fixedSupply";
      msg: "Fixed supply";
    },
    {
      code: 6;
      name: "alreadyInUse";
      msg: "Already in use";
    },
    {
      code: 7;
      name: "invalidNumberOfProvidedSigners";
      msg: "Invalid number of provided signers";
    },
    {
      code: 8;
      name: "invalidNumberOfRequiredSigners";
      msg: "Invalid number of required signers";
    },
    {
      code: 9;
      name: "uninitializedState";
      msg: "State is unititialized";
    },
    {
      code: 10;
      name: "nativeNotSupported";
      msg: "Instruction does not support native tokens";
    },
    {
      code: 11;
      name: "nonNativeHasBalance";
      msg: "Non-native account can only be closed if its balance is zero";
    },
    {
      code: 12;
      name: "invalidInstruction";
      msg: "Invalid instruction";
    },
    {
      code: 13;
      name: "invalidState";
      msg: "State is invalid for requested operation";
    },
    {
      code: 14;
      name: "overflow";
      msg: "Operation overflowed";
    },
    {
      code: 15;
      name: "authorityTypeNotSupported";
      msg: "Account does not support specified authority type";
    },
    {
      code: 16;
      name: "mintCannotFreeze";
      msg: "This token mint cannot freeze accounts";
    },
    {
      code: 17;
      name: "accountFrozen";
      msg: "Account is frozen";
    },
    {
      code: 18;
      name: "mintDecimalsMismatch";
      msg: "The provided decimals value different from the Mint decimals";
    },
    {
      code: 19;
      name: "nonNativeNotSupported";
      msg: "Instruction does not support non-native tokens";
    }
  ];
  types: [
    {
      name: "accountState";
      type: {
        kind: "enum";
        variants: [
          {
            name: "uninitialized";
          },
          {
            name: "initialized";
          },
          {
            name: "frozen";
          }
        ];
      };
    },
    {
      name: "authorityType";
      type: {
        kind: "enum";
        variants: [
          {
            name: "mintTokens";
          },
          {
            name: "freezeAccount";
          },
          {
            name: "accountOwner";
          },
          {
            name: "closeAccount";
          }
        ];
      };
    },
    {
      name: "mint";
      type: {
        kind: "struct";
        fields: [
          {
            name: "mintAuthority";
            docs: [
              "Optional authority used to mint new tokens. The mint authority may only be provided during",
              "mint creation. If no mint authority is present then the mint has a fixed supply and no",
              "further tokens may be minted."
            ];
            type: {
              coption: "pubkey";
            };
          },
          {
            name: "supply";
            docs: ["Total supply of tokens."];
            type: "u64";
          },
          {
            name: "decimals";
            docs: [
              "Number of base 10 digits to the right of the decimal place."
            ];
            type: "u8";
          },
          {
            name: "isInitialized";
            docs: ["Is `true` if this structure has been initialized"];
            type: "bool";
          },
          {
            name: "freezeAuthority";
            docs: ["Optional authority to freeze token accounts."];
            type: {
              coption: "pubkey";
            };
          }
        ];
      };
    },
    {
      name: "multisig";
      type: {
        kind: "struct";
        fields: [
          {
            name: "m";
            docs: ["Number of signers required"];
            type: "u8";
          },
          {
            name: "n";
            docs: ["Number of valid signers"];
            type: "u8";
          },
          {
            name: "isInitialized";
            docs: ["Is `true` if this structure has been initialized"];
            type: "bool";
          },
          {
            name: "signers";
            docs: ["Signer public keys"];
            type: {
              array: ["pubkey", 11];
            };
          }
        ];
      };
    },
    {
      name: "account";
      type: {
        kind: "struct";
        fields: [
          {
            name: "mint";
            docs: ["The mint associated with this account"];
            type: "pubkey";
          },
          {
            name: "owner";
            docs: ["The owner of this account."];
            type: "pubkey";
          },
          {
            name: "amount";
            docs: ["The amount of tokens this account holds."];
            type: "u64";
          },
          {
            name: "delegate";
            docs: [
              "If `delegate` is `Some` then `delegated_amount` represents",
              "the amount authorized by the delegate"
            ];
            type: {
              coption: "pubkey";
            };
          },
          {
            name: "state";
            docs: ["The account's state"];
            type: {
              defined: {
                name: "accountState";
              };
            };
          },
          {
            name: "isNative";
            docs: [
              "If is_native.is_some, this is a native token, and the value logs the rent-exempt reserve. An",
              "Account is required to be rent-exempt, so the value is used by the Processor to ensure that",
              "wrapped SOL accounts do not drop below this threshold."
            ];
            type: {
              coption: "u64";
            };
          },
          {
            name: "delegatedAmount";
            docs: ["The amount delegated"];
            type: "u64";
          },
          {
            name: "closeAuthority";
            docs: ["Optional authority to close the account."];
            type: {
              coption: "pubkey";
            };
          }
        ];
      };
    }
  ];
};

const IDL: SplToken = {
  address: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
  metadata: {
    name: "splToken",
    version: "3.3.0",
    spec: "0.1.0",
  },
  instructions: [
    {
      name: "amountToUiAmount",
      discriminator: [23],
      accounts: [
        {
          name: "mint",
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
      ],
    },
    {
      name: "approve",
      discriminator: [4],
      accounts: [
        {
          name: "source",
          writable: true,
        },
        {
          name: "delegate",
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
      ],
    },
    {
      name: "approveChecked",
      discriminator: [13],
      accounts: [
        {
          name: "source",
          writable: true,
        },
        {
          name: "mint",
        },
        {
          name: "delegate",
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
        {
          name: "decimals",
          type: "u8",
        },
      ],
    },
    {
      name: "burn",
      discriminator: [8],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
          writable: true,
        },
        {
          name: "authority",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
      ],
    },
    {
      name: "burnChecked",
      discriminator: [15],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
          writable: true,
        },
        {
          name: "authority",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
        {
          name: "decimals",
          type: "u8",
        },
      ],
    },
    {
      name: "closeAccount",
      discriminator: [9],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "destination",
          writable: true,
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [],
    },
    {
      name: "freezeAccount",
      discriminator: [10],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [],
    },
    {
      name: "getAccountDataSize",
      discriminator: [21],
      accounts: [
        {
          name: "mint",
        },
      ],
      args: [],
    },
    {
      name: "initializeAccount",
      discriminator: [1],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
        },
        {
          name: "owner",
        },
        {
          name: "rent",
          address: "SysvarRent111111111111111111111111111111111",
        },
      ],
      args: [],
    },
    {
      name: "initializeAccount2",
      discriminator: [16],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
        },
        {
          name: "rent",
          address: "SysvarRent111111111111111111111111111111111",
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
      name: "initializeAccount3",
      discriminator: [18],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
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
      name: "initializeImmutableOwner",
      discriminator: [22],
      accounts: [
        {
          name: "account",
          writable: true,
        },
      ],
      args: [],
    },
    {
      name: "initializeMint",
      discriminator: [0],
      accounts: [
        {
          name: "mint",
          writable: true,
        },
        {
          name: "rent",
          address: "SysvarRent111111111111111111111111111111111",
        },
      ],
      args: [
        {
          name: "decimals",
          type: "u8",
        },
        {
          name: "mintAuthority",
          type: "pubkey",
        },
        {
          name: "freezeAuthority",
          type: {
            coption: "pubkey",
          },
        },
      ],
    },
    {
      name: "initializeMint2",
      discriminator: [20],
      accounts: [
        {
          name: "mint",
          writable: true,
        },
      ],
      args: [
        {
          name: "decimals",
          type: "u8",
        },
        {
          name: "mintAuthority",
          type: "pubkey",
        },
        {
          name: "freezeAuthority",
          type: {
            coption: "pubkey",
          },
        },
      ],
    },
    {
      name: "initializeMultisig",
      discriminator: [2],
      accounts: [
        {
          name: "multisig",
          writable: true,
        },
        {
          name: "rent",
          address: "SysvarRent111111111111111111111111111111111",
        },
      ],
      args: [
        {
          name: "m",
          type: "u8",
        },
      ],
    },
    {
      name: "initializeMultisig2",
      discriminator: [19],
      accounts: [
        {
          name: "multisig",
          writable: true,
        },
        {
          name: "signer",
        },
      ],
      args: [
        {
          name: "m",
          type: "u8",
        },
      ],
    },
    {
      name: "mintTo",
      discriminator: [7],
      accounts: [
        {
          name: "mint",
          writable: true,
        },
        {
          name: "account",
          writable: true,
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
      ],
    },
    {
      name: "mintToChecked",
      discriminator: [14],
      accounts: [
        {
          name: "mint",
          writable: true,
        },
        {
          name: "account",
          writable: true,
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
        {
          name: "decimals",
          type: "u8",
        },
      ],
    },
    {
      name: "revoke",
      discriminator: [5],
      accounts: [
        {
          name: "source",
          writable: true,
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [],
    },
    {
      name: "setAuthority",
      discriminator: [6],
      accounts: [
        {
          name: "owned",
          writable: true,
        },
        {
          name: "owner",
          signer: true,
        },
        {
          name: "signer",
          signer: true,
        },
      ],
      args: [
        {
          name: "authorityType",
          type: {
            defined: {
              name: "authorityType",
            },
          },
        },
        {
          name: "newAuthority",
          type: {
            coption: "pubkey",
          },
        },
      ],
    },
    {
      name: "syncNative",
      discriminator: [17],
      accounts: [
        {
          name: "account",
          writable: true,
        },
      ],
      args: [],
    },
    {
      name: "thawAccount",
      discriminator: [11],
      accounts: [
        {
          name: "account",
          writable: true,
        },
        {
          name: "mint",
        },
        {
          name: "owner",
          signer: true,
        },
      ],
      args: [],
    },
    {
      name: "transfer",
      discriminator: [3],
      accounts: [
        {
          name: "source",
          writable: true,
        },
        {
          name: "destination",
          writable: true,
        },
        {
          name: "authority",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
      ],
    },
    {
      name: "transferChecked",
      discriminator: [12],
      accounts: [
        {
          name: "source",
          writable: true,
        },
        {
          name: "mint",
        },
        {
          name: "destination",
          writable: true,
        },
        {
          name: "authority",
          signer: true,
        },
      ],
      args: [
        {
          name: "amount",
          type: "u64",
        },
        {
          name: "decimals",
          type: "u8",
        },
      ],
    },
    {
      name: "uiAmountToAmount",
      discriminator: [24],
      accounts: [
        {
          name: "mint",
        },
      ],
      args: [
        {
          name: "uiAmount",
          type: "string",
        },
      ],
    },
  ],
  accounts: [
    {
      name: "mint",
      discriminator: [],
    },
    {
      name: "multisig",
      discriminator: [],
    },
    {
      name: "account",
      discriminator: [],
    },
  ],
  errors: [
    {
      code: 0,
      name: "notRentExempt",
      msg: "Lamport balance below rent-exempt threshold",
    },
    {
      code: 1,
      name: "insufficientFunds",
      msg: "Insufficient funds",
    },
    {
      code: 2,
      name: "invalidMint",
      msg: "Invalid Mint",
    },
    {
      code: 3,
      name: "mintMismatch",
      msg: "Account not associated with this Mint",
    },
    {
      code: 4,
      name: "ownerMismatch",
      msg: "Owner does not match",
    },
    {
      code: 5,
      name: "fixedSupply",
      msg: "Fixed supply",
    },
    {
      code: 6,
      name: "alreadyInUse",
      msg: "Already in use",
    },
    {
      code: 7,
      name: "invalidNumberOfProvidedSigners",
      msg: "Invalid number of provided signers",
    },
    {
      code: 8,
      name: "invalidNumberOfRequiredSigners",
      msg: "Invalid number of required signers",
    },
    {
      code: 9,
      name: "uninitializedState",
      msg: "State is unititialized",
    },
    {
      code: 10,
      name: "nativeNotSupported",
      msg: "Instruction does not support native tokens",
    },
    {
      code: 11,
      name: "nonNativeHasBalance",
      msg: "Non-native account can only be closed if its balance is zero",
    },
    {
      code: 12,
      name: "invalidInstruction",
      msg: "Invalid instruction",
    },
    {
      code: 13,
      name: "invalidState",
      msg: "State is invalid for requested operation",
    },
    {
      code: 14,
      name: "overflow",
      msg: "Operation overflowed",
    },
    {
      code: 15,
      name: "authorityTypeNotSupported",
      msg: "Account does not support specified authority type",
    },
    {
      code: 16,
      name: "mintCannotFreeze",
      msg: "This token mint cannot freeze accounts",
    },
    {
      code: 17,
      name: "accountFrozen",
      msg: "Account is frozen",
    },
    {
      code: 18,
      name: "mintDecimalsMismatch",
      msg: "The provided decimals value different from the Mint decimals",
    },
    {
      code: 19,
      name: "nonNativeNotSupported",
      msg: "Instruction does not support non-native tokens",
    },
  ],
  types: [
    {
      name: "accountState",
      type: {
        kind: "enum",
        variants: [
          {
            name: "uninitialized",
          },
          {
            name: "initialized",
          },
          {
            name: "frozen",
          },
        ],
      },
    },
    {
      name: "authorityType",
      type: {
        kind: "enum",
        variants: [
          {
            name: "mintTokens",
          },
          {
            name: "freezeAccount",
          },
          {
            name: "accountOwner",
          },
          {
            name: "closeAccount",
          },
        ],
      },
    },
    {
      name: "mint",
      type: {
        kind: "struct",
        fields: [
          {
            name: "mintAuthority",
            docs: [
              "Optional authority used to mint new tokens. The mint authority may only be provided during",
              "mint creation. If no mint authority is present then the mint has a fixed supply and no",
              "further tokens may be minted.",
            ],
            type: {
              coption: "pubkey",
            },
          },
          {
            name: "supply",
            docs: ["Total supply of tokens."],
            type: "u64",
          },
          {
            name: "decimals",
            docs: [
              "Number of base 10 digits to the right of the decimal place.",
            ],
            type: "u8",
          },
          {
            name: "isInitialized",
            docs: ["Is `true` if this structure has been initialized"],
            type: "bool",
          },
          {
            name: "freezeAuthority",
            docs: ["Optional authority to freeze token accounts."],
            type: {
              coption: "pubkey",
            },
          },
        ],
      },
    },
    {
      name: "multisig",
      type: {
        kind: "struct",
        fields: [
          {
            name: "m",
            docs: ["Number of signers required"],
            type: "u8",
          },
          {
            name: "n",
            docs: ["Number of valid signers"],
            type: "u8",
          },
          {
            name: "isInitialized",
            docs: ["Is `true` if this structure has been initialized"],
            type: "bool",
          },
          {
            name: "signers",
            docs: ["Signer public keys"],
            type: {
              array: ["pubkey", 11],
            },
          },
        ],
      },
    },
    {
      name: "account",
      type: {
        kind: "struct",
        fields: [
          {
            name: "mint",
            docs: ["The mint associated with this account"],
            type: "pubkey",
          },
          {
            name: "owner",
            docs: ["The owner of this account."],
            type: "pubkey",
          },
          {
            name: "amount",
            docs: ["The amount of tokens this account holds."],
            type: "u64",
          },
          {
            name: "delegate",
            docs: [
              "If `delegate` is `Some` then `delegated_amount` represents",
              "the amount authorized by the delegate",
            ],
            type: {
              coption: "pubkey",
            },
          },
          {
            name: "state",
            docs: ["The account's state"],
            type: {
              defined: {
                name: "accountState",
              },
            },
          },
          {
            name: "isNative",
            docs: [
              "If is_native.is_some, this is a native token, and the value logs the rent-exempt reserve. An",
              "Account is required to be rent-exempt, so the value is used by the Processor to ensure that",
              "wrapped SOL accounts do not drop below this threshold.",
            ],
            type: {
              coption: "u64",
            },
          },
          {
            name: "delegatedAmount",
            docs: ["The amount delegated"],
            type: "u64",
          },
          {
            name: "closeAuthority",
            docs: ["Optional authority to close the account."],
            type: {
              coption: "pubkey",
            },
          },
        ],
      },
    },
  ],
};
