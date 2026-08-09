use std::{fs, path::PathBuf, process::Command};

use anchor_lang_v2::{wincode, InitSpace, Space};

#[derive(InitSpace, wincode::SchemaRead, wincode::SchemaWrite)]
#[wincode(tag_encoding = "u32")]
enum WideTagVariant {
    A,
    B(u64),
    C { _x: u16 },
}

#[test]
fn init_space_honors_wincode_tag_encoding() {
    assert_eq!(WideTagVariant::INIT_SPACE, 4 + 8);
}

fn cargo_test_case(name: &str, source: &str, features: &[&str], extra_files: &[(&str, &str)]) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir.join("target/compile-cases").join(name);
    let src_dir = crate_dir.join("src");
    let anchor_lang_v2 = manifest_dir
        .parent()
        .expect("tests-v2 should live under the workspace root")
        .join("lang-v2");

    if crate_dir.exists() {
        fs::remove_dir_all(&crate_dir).unwrap();
    }
    fs::create_dir_all(&src_dir).unwrap();

    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
anchor-lang-v2 = {{ path = "{}" }}
wincode = {{ version = "0.5", features = ["derive"] }}

[features]
idl-build = []

[workspace]
"#,
            anchor_lang_v2.display()
        ),
    )
    .unwrap();
    fs::write(src_dir.join("lib.rs"), source).unwrap();

    for (relative_path, contents) in extra_files {
        let path = crate_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    let mut command = Command::new("cargo");
    command.args(["test", "--offline", "--manifest-path"]);
    command.arg(crate_dir.join("Cargo.toml"));
    if !features.is_empty() {
        command.arg("--features");
        command.arg(features.join(","));
    }

    let output = command
        .output()
        .unwrap_or_else(|err| panic!("failed to run cargo test for {name}: {err}"));

    assert!(
        output.status.success(),
        "{name} tests failed\n\nstdout:\n{}\n\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn emitted_idl_keeps_wincode_tag_metadata() {
    cargo_test_case(
        "tests_v2_idl_enum_wincode_tags",
        r#"
use anchor_lang_v2::{IdlAccountType, IdlType};

#[derive(Clone, IdlType, wincode::SchemaRead, wincode::SchemaWrite)]
#[wincode(tag_encoding = "u32")]
pub enum Tagged {
    #[wincode(tag = 5)]
    A,
    #[wincode(tag = 8)]
    B(u64),
}

#[cfg(all(test, feature = "idl-build"))]
mod tests {
    use super::*;

    #[test]
    fn emitted_idl_keeps_wincode_tag_metadata() {
        let idl = <Tagged as IdlAccountType>::__idl_type_def().unwrap();
        assert!(idl.contains("\"tagEncoding\":\"u32\""));
        assert!(idl.contains("\"name\":\"A\""));
        assert!(idl.contains("\"tag\":\"5\""));
        assert!(idl.contains("\"tag\":\"8\""));
    }
}
"#,
        &["idl-build"],
        &[],
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn declare_program_round_trips_wincode_enum_tags() {
    cargo_test_case(
        "tests_v2_declare_program_wincode_tags",
        r#"
use anchor_lang_v2::declare_program;

declare_program!(tagged_program);

#[cfg(test)]
mod tests {
    use super::tagged_program::types::Tagged;

    #[test]
    fn declared_enum_roundtrips_with_custom_tags() {
        let a_bytes = anchor_lang_v2::wincode::config::serialize(
            &Tagged::A,
            anchor_lang_v2::BORSH_CONFIG,
        )
        .unwrap();
        assert_eq!(a_bytes, 5u32.to_le_bytes());

        let b_bytes = anchor_lang_v2::wincode::config::serialize(
            &Tagged::B(9),
            anchor_lang_v2::BORSH_CONFIG,
        )
        .unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(&8u32.to_le_bytes());
        expected.extend_from_slice(&9u64.to_le_bytes());
        assert_eq!(b_bytes, expected);
    }
}
"#,
        &[],
        &[(
            "idls/tagged_program.json",
            r#"{
  "address": "11111111111111111111111111111111",
  "metadata": {
    "name": "tagged_program",
    "version": "0.1.0",
    "spec": "0.1.0"
  },
  "instructions": [],
  "accounts": [],
  "types": [
    {
      "name": "Tagged",
      "type": {
        "kind": "enum",
        "tagEncoding": "u32",
        "variants": [
          { "name": "A", "tag": "5" },
          { "name": "B", "tag": "8", "fields": ["u64"] }
        ]
      }
    }
  ]
}"#,
        )],
    );
}
