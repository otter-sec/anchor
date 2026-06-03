/**
 * Generate instructions with repetitive accounts and add them to the bench program.
 */

import * as fs from "fs/promises";
import path from "path";

type Instruction = {
  /** Instruction name */
  name: string;
  /** Each account type in accounts struct */
  accountType: string;
  /** Account macro(`#[account(..)]`) */
  accountMacro?: {
    init: true;
    space?: number | string;
  };
  /** Number of accounts to create per instruction */
  accountCounts?: number[];
};

/**
 * The following instructions will be added to the program.
 *
 * If an instruction already exists, it will be skipped.
 */
const INSTRUCTIONS: Instruction[] = [
  {
    name: "account_info",
    accountType: "AccountInfo<'info>",
  },
  {
    name: "account_empty_init",
    accountType: "Account<'info, Empty>",
    accountMacro: {
      init: true,
    },
  },
  {
    name: "account_empty",
    accountType: "Account<'info, Empty>",
  },
  {
    name: "account_sized_init",
    accountType: "Account<'info, Sized>",
    accountMacro: {
      init: true,
      space: "8 + std::mem::size_of::<Sized>()",
    },
  },
  {
    name: "account_sized",
    accountType: "Account<'info, Sized>",
  },
  {
    name: "account_unsized_init",
    accountType: "Account<'info, Unsized>",
    accountMacro: {
      init: true,
      space: "8 + std::mem::size_of::<Unsized>()",
    },
  },
  {
    name: "account_unsized",
    accountType: "Account<'info, Unsized>",
  },
  {
    name: "boxed_account_empty_init",
    accountType: "Box<Account<'info, Empty>>",
    accountMacro: {
      init: true,
    },
  },
  {
    name: "boxed_account_empty",
    accountType: "Box<Account<'info, Empty>>",
  },
  {
    name: "boxed_account_sized_init",
    accountType: "Box<Account<'info, Sized>>",
    accountMacro: {
      init: true,
      space: "8 + std::mem::size_of::<Sized>()",
    },
  },
  {
    name: "boxed_account_sized",
    accountType: "Box<Account<'info, Sized>>",
  },
  {
    name: "boxed_account_unsized_init",
    accountType: "Box<Account<'info, Unsized>>",
    accountMacro: {
      init: true,
      space: "8 + std::mem::size_of::<Unsized>()",
    },
  },
  {
    name: "boxed_account_unsized",
    accountType: "Box<Account<'info, Unsized>>",
  },
  {
    name: "boxed_interface_account_mint",
    accountType: "Box<InterfaceAccount<'info, Mint>>",
  },
  {
    name: "boxed_interface_account_token",
    accountType: "Box<InterfaceAccount<'info, TokenAccount>>",
  },
  {
    name: "interface_account_mint",
    accountType: "InterfaceAccount<'info, Mint>",
  },
  {
    name: "interface_account_token",
    accountType: "InterfaceAccount<'info, TokenAccount>",
    accountCounts: [1, 2, 4],
  },
  {
    name: "interface",
    accountType: "Interface<'info, TokenInterface>",
  },
  {
    name: "program",
    accountType: "Program<'info, System>",
  },
  {
    name: "signer",
    accountType: "Signer<'info>",
  },
  {
    name: "system_account",
    accountType: "SystemAccount<'info>",
  },
  {
    name: "unchecked_account",
    accountType: "UncheckedAccount<'info>",
  },
];

(async () => {
  // Get the program file
  const programPath = path.join("programs", "bench", "src", "lib.rs");
  let file = await fs.readFile(programPath, {
    encoding: "utf8",
  });

  const create = (
    ix: Omit<Instruction, "accountCounts"> & { count: number }
  ) => {
    // Get the title case of the name for the accounts struct
    const accountsName =
      ix.name[0].toUpperCase() +
      ix.name.slice(1).replace(/_\w/g, (match) => match[1].toUpperCase());

    // Generate accounts
    let accounts = "";
    let accountMacro = "";
    const INDENT = "\n    ";

    if (ix.accountMacro?.init) {
      accounts += `${INDENT}#[account(mut)]${INDENT}pub payer: Signer<'info>,`;
      accounts += `${INDENT}pub system_program: Program<'info, System>,`;
      accountMacro += `init, payer = payer, space = ${
        ix.accountMacro.space ?? 8
      }`;
    }

    accountMacro = `${INDENT}#[account(${accountMacro})]`;

    for (let i = 0; i < ix.count; i++) {
      if (ix.accountMacro) {
        accounts += accountMacro;
      }

      accounts += `${INDENT}pub account${i + 1}: ${ix.accountType},`;
    }

    return {
      ix: `
    pub fn ${ix.name}(_ctx: Context<${accountsName}>) -> Result<()> {
        Ok(())
    }`,
      accounts: `
#[derive(Accounts)]
pub struct ${accountsName}<'info> {${accounts}\n}`,
    };
  };

  const insert = (index: number, text: string) => {
    file = file.slice(0, index) + "\n" + text + file.slice(index);
  };

  for (const instruction of INSTRUCTIONS) {
    // Default count
    instruction.accountCounts ??= [1, 2, 4, 8];

    for (const count of instruction.accountCounts) {
      // Append count to the end of the instruction name
      const ixName = instruction.name + count;

      // Skip existing instructions
      if (file.includes(`fn ${ixName}`)) {
        continue;
      }

      const { ix, accounts } = create({ ...instruction, name: ixName, count });

      // Get the ix index to start from
      const programIndex = file.indexOf("#[program]");
      const fileStartingFromProgram = file.slice(programIndex);

      // Add instruction
      const ixIndex = programIndex + fileStartingFromProgram.indexOf("\n}");
      insert(ixIndex, ix);

      // Add accounts
      const accountsIndex = file.length - 1;
      insert(accountsIndex, accounts);
    }
  }

  // Save
  await fs.writeFile(programPath, file);
})();
