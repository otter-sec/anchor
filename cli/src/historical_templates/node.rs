use {
    crate::template::AnchorVersion,
    heck::{ToPascalCase, ToSnakeCase},
};

pub(super) fn test(name: &str, javascript: bool, jest: bool, version: AnchorVersion) -> String {
    let snake = name.to_snake_case();
    let pascal = name.to_pascal_case();
    let imports = if javascript {
        format!(
            "const anchor = require(\"@anchor-lang/core\");\nconst {{ HistoricalFork }} = \
             require(\"./historical\");\nconst idl = require(\"../target/idl/{snake}.json\");\n"
        )
    } else {
        format!(
            "import * as anchor from \"@anchor-lang/core\";\nimport {{ HistoricalFork }} from \
             \"./historical\";\nimport type {{ {pascal} }} from \
             \"../target/types/{snake}\";\nimport idl from \"../target/idl/{snake}.json\";\n"
        )
    };
    let declarations = if javascript {
        "let fork;\n  let program;\n  let counter;".to_owned()
    } else {
        format!(
            "let fork: HistoricalFork | undefined;\n  let program: anchor.Program<{pascal}>;\n  \
             let counter: anchor.web3.Keypair;"
        )
    };
    let before = if jest { "beforeAll" } else { "before" };
    let after = if jest { "afterAll" } else { "after" };
    let hook_timeout = if jest { ", 300_000" } else { "" };
    let call_timeout = if jest { ", 60_000" } else { "" };
    let counter = match version {
        AnchorVersion::V1 => {
            r#"const [counterAddress] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("counter")], program.programId,
    );"#
        }
        AnchorVersion::V2 => {
            "counter = anchor.web3.Keypair.generate();\n    const counterAddress = \
             counter.publicKey;"
        }
    };
    let program_type = if javascript {
        String::new()
    } else {
        format!(" as anchor.Program<{pascal}>")
    };
    let body = match version {
        AnchorVersion::V1 => {
            r#"await program.methods.initialize().rpc();
    await program.methods.increment().rpc();"#
        }
        AnchorVersion::V2 => {
            r#"await program.methods.initialize()
      .accounts({ counter: counter.publicKey })
      .signers([counter])
      .rpc();"#
        }
    };
    format!(
        r#"{imports}
describe("{name} — historical fork", () => {{
  {declarations}

  {before}(async () => {{
    // The historical boundary (a transaction signature or slot) is chosen here,
    // in test code.
    fork = await HistoricalFork.start({{ after: "413023260" }});
    const provider = fork.provider();
    program = new anchor.Program(idl, provider){program_type};
    // Hypothetical, fork-local state: fund the wallet and install this build of
    // the program with
    // Surfpool's fast program-writing helper (not a loader deployment).
    await fork.setAccount(provider.wallet.publicKey, {{ lamports: 10_000_000_000 }});
    await fork.installProgram(program.programId, "target/deploy/{snake}.so");
    {counter}
    // The account the test creates did not exist at the boundary.
    await fork.setAccount(counterAddress);
    // The clock is frozen: advance one slot so the installed program becomes
    // callable.
    await fork.advance(1);
  }}{hook_timeout});

  {after}(async () => {{
    await fork?.close();
  }}{call_timeout});

  it("executes the program against historical state", async () => {{
    {body}
  }}{call_timeout});
}});
"#
    )
}
