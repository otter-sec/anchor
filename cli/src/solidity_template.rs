use crate::create_files;
use anyhow::Result;
use heck::{ToSnakeCase, ToUpperCamelCase};
use std::path::Path;

/// Create a solidity program.
pub fn create_program(name: &str) -> Result<()> {
    let files = vec![(
        Path::new("solidity").join(name).with_extension("sol"),
        solidity(name),
    )];
    create_files(&files)
}

fn solidity(name: &str) -> String {
    format!(
        r#"
contract {} {{
    bool private value = true;

    @payer(payer)
    constructor() {{
        print("Hello, World!");
    }}

    /// A message that can be called on instantiated contracts.
    /// This one flips the value of the stored `bool` from `true`
    /// to `false` and vice versa.
    function flip() public {{
            value = !value;
    }}

    /// Simply returns the current value of our `bool`.
    function get() public view returns (bool) {{
            return value;
    }}
}}
"#,
        name.to_snake_case(),
    )
}

pub fn mocha(name: &str) -> String {
    format!(
        r#"const anchor = require("@anchor-lang/core");

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  it("Is initialized!", async () => {{
    // Add your test here.
    const program = anchor.workspace.{};
    const dataAccount = anchor.web3.Keypair.generate();

    const tx = await program.methods
       .new()
       .accounts({{ dataAccount: dataAccount.publicKey }})
       .signers([dataAccount])
       .rpc();

    console.log("Your transaction signature", tx);

    const val1 = await program.methods
      .get()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .view();

    console.log("state", val1);

    await program.methods
      .flip()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .rpc();

    const val2 = await program.methods
      .get()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .view();

    console.log("state", val2);
  }});
}});
"#,
        name,
        name.to_upper_camel_case(),
    )
}

pub fn jest(name: &str) -> String {
    format!(
        r#"const anchor = require("@anchor-lang/core");

describe("{}", () => {{
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  it("Is initialized!", async () => {{
    // Add your test here.
    const program = anchor.workspace.{};
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  }});
}});
"#,
        name,
        name.to_upper_camel_case(),
    )
}

pub fn ts_mocha(name: &str) -> String {
    format!(
        r#"import * as anchor from "@anchor-lang/core";
import {{ Program }} from "@anchor-lang/core";
import {{ {} }} from "../target/types/{}";

describe("{}", () => {{
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const dataAccount = anchor.web3.Keypair.generate();

  const program = anchor.workspace.{} as Program<{}>;

  it("Is initialized!", async () => {{
    // Add your test here.
    const tx = await program.methods
      .new()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .signers([dataAccount])
      .rpc();
    console.log("Your transaction signature", tx);

    const val1 = await program.methods
      .get()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .view();

    console.log("state", val1);

    await program.methods
      .flip()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .rpc();

    const val2 = await program.methods
      .get()
      .accounts({{ dataAccount: dataAccount.publicKey }})
      .view();

    console.log("state", val2);  }});
}});
"#,
        name.to_upper_camel_case(),
        name.to_snake_case(),
        name,
        name.to_upper_camel_case(),
        name.to_upper_camel_case(),
    )
}
