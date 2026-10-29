use {crate::template::AnchorVersion, anyhow::Result, heck::ToSnakeCase, std::fs};

pub(super) fn write(name: &str, version: AnchorVersion) -> Result<()> {
    let name = name.to_snake_case();
    let manifest = fs::read_to_string("tests/Cargo.toml")?;
    // The helper derives the installed program's programdata address.
    let pubkey = "solana-pubkey = \"3.0.0\"";
    if !manifest.contains(pubkey) {
        anyhow::bail!("missing `{pubkey}` in tests/Cargo.toml");
    }
    let manifest = manifest.replacen(
        pubkey,
        "solana-pubkey = { version = \"3.0.0\", features = [\"curve25519\"] }",
        1,
    );
    fs::write(
        "tests/Cargo.toml",
        format!(
            "{manifest}\nserde_json = \"1\"\nreqwest = {{ version = \"0.12\", default-features = \
             false, features = [\"blocking\", \"json\"] }}\n"
        ),
    )?;
    fs::write(
        "tests/src/lib.rs",
        "#[cfg(test)]\nmod historical;\n#[cfg(test)]\nmod test_initialize;\n",
    )?;
    fs::write(
        "tests/src/historical.rs",
        include_str!("historical_fork.rs.txt"),
    )?;
    let old = fs::read_to_string("tests/src/test_initialize.rs")?;
    // Keep the generated program-specific accounts/instructions unchanged.
    let start = old
        .find("    let anchor_wallet =")
        .ok_or_else(|| anyhow::anyhow!("missing Rust wallet setup"))?;
    let end = old[start..]
        .find("    let client =")
        .ok_or_else(|| anyhow::anyhow!("missing Rust client setup"))?
        + start;
    let counter = if version == AnchorVersion::V2 {
        "    let counter = Keypair::new();\n"
    } else {
        ""
    };
    let setup = format!(
        r#"    // The historical boundary (a transaction signature or slot) is chosen here, in test code.
    let mut fork = super::historical::HistoricalFork::start("413023260", None).expect("historical fork");
    let payer = solana_keypair::Keypair::new();
    fork.set_account(&payer.pubkey().to_string(), 10_000_000_000).expect("fund the payer in this fork");
    let program_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/deploy/{name}.so");
    // Fast ELF setup, not a loader deployment.
    fork.install_program(program_id, &program_path, &payer.pubkey().to_string()).expect("install the program");
{counter}"#
    );
    let test = format!("{}{setup}{}", &old[..start], &old[end..]);
    let test = test.replace("Cluster::Localnet", "fork.cluster()");
    let test = test.replace("use solana_keypair::read_keypair_file;\n", "");
    let test = test.replace(
        "use solana_keypair::{read_keypair_file, Keypair};",
        "use solana_keypair::Keypair;",
    );
    let marker = match version {
        AnchorVersion::V1 => "    let initialize_tx =",
        AnchorVersion::V2 => "    let tx =",
    };
    let counter = match version {
        AnchorVersion::V1 => "&counter.to_string()",
        AnchorVersion::V2 => "&counter.pubkey().to_string()",
    };
    // The account the test creates did not exist at the boundary; the explicit
    // advance makes the
    // program installed in this slot callable.
    let test = test.replacen(
        marker,
        &format!(
            "    fork.set_account({counter}, 0).expect(\"declare the new counter\");\n    \
             fork.advance(1).expect(\"advance one slot\");\n{marker}"
        ),
        1,
    );
    // Close the fork explicitly so a cleanup failure fails the test.
    let close = test
        .rfind("\n}")
        .ok_or_else(|| anyhow::anyhow!("missing Rust test end"))?;
    let test = format!(
        "{}\n    fork.close().expect(\"close the historical fork\");{}",
        &test[..close],
        &test[close..]
    );
    fs::write("tests/src/test_initialize.rs", test)?;
    Ok(())
}
