//! Type aliases and renamed imports are referenced in the IDL by the name the
//! type is registered under in `types[]`. Builds a real program IDL, checks
//! every reference, then declares the program from that IDL.

use {
    anchor_lang_idl_spec::{IdlDefinedFields, IdlGenericArg, IdlType, IdlTypeDef, IdlTypeDefTy},
    std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    },
};

const MODELS: &str = r#"use anchor_lang::prelude::*;

declare_id!("Hy6xBbKVJBPx5PV7VsUqrJxGaRFgbmyJkYyzc1L6YMmU");

pub mod models {
    use super::*;

    #[derive(Clone, IdlType, AnchorSerialize, AnchorDeserialize)]
    pub struct BumpArgs {
        pub amount: u64,
    }

    #[derive(Clone, IdlType, AnchorSerialize, AnchorDeserialize)]
    pub struct Wrapper<T: IdlAccountType> {
        pub value: T,
    }
}
"#;

const PROGRAM: &str = r#"
use models::BumpArgs as Renamed;

pub type Aliased = models::BumpArgs;

#[derive(Clone, IdlType, AnchorSerialize, AnchorDeserialize)]
pub struct Holder {
    pub aliased: Aliased,
    pub renamed: Renamed,
    pub list: Vec<Aliased>,
    pub wrapped: models::Wrapper<Renamed>,
}

#[program]
pub mod aliases {
    use super::*;

    #[discrim = 0]
    pub fn run(_ctx: &mut Context<Run>, args: Aliased, holder: Holder) -> Result<Renamed> {
        let _ = holder;
        Ok(args)
    }
}

#[derive(Accounts)]
pub struct Run {
    pub signer: Signer,
}
"#;

const GENERIC_ALIAS_PROGRAM: &str = r#"
pub type GenericAlias = models::Wrapper<u64>;

#[program]
pub mod aliases {
    use super::*;

    #[discrim = 0]
    pub fn run(_ctx: &mut Context<Run>, args: GenericAlias) -> Result<()> {
        let _ = args;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Run {
    pub signer: Signer,
}
"#;

const CONSUMER: &str = r#"use anchor_lang::prelude::*;

declare_id!("11111111111111111111111111111111");

declare_program!(aliases);

#[test]
fn declared_aliases_use_the_registered_types() {
    use aliases::types::{BumpArgs, Holder, Wrapper};

    let _ = Holder {
        aliased: BumpArgs { amount: 1 },
        renamed: BumpArgs { amount: 2 },
        list: vec![BumpArgs { amount: 3 }],
        wrapped: Wrapper {
            value: BumpArgs { amount: 4 },
        },
    };
}
"#;

fn write_crate(dir: &Path, name: &str, anchor_lang: &Path, features: &str, source: &str) {
    if dir.exists() {
        fs::remove_dir_all(dir).unwrap();
    }
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
publish = false

[features]
{features}

[dependencies]
anchor-lang = {{ path = "{}" }}

[lints.rust.unexpected_cfgs]
level = "allow"

[workspace]
"#,
            anchor_lang.display()
        ),
    )
    .unwrap();
    fs::write(dir.join("src/lib.rs"), source).unwrap();
}

fn cargo_test(dir: &Path, target_dir: &Path, args: &[&str]) -> Output {
    Command::new("cargo")
        .arg("test")
        .args(args)
        .args(["--offline", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .args(["--", "--show-output", "--quiet", "--test-threads=1"])
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .unwrap()
}

fn assert_success(dir: &Path, output: &Output) {
    assert!(
        output.status.success(),
        "cargo test failed in {}\n\nstdout:\n{}\n\nstderr:\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn idl_block(stdout: &str, name: &str) -> String {
    stdout
        .split(&format!("--- IDL begin {name} ---\n"))
        .nth(1)
        .and_then(|block| block.split(&format!("\n--- IDL end {name} ---")).next())
        .unwrap_or_else(|| panic!("{name} IDL block should be printed"))
        .to_owned()
}

fn print_idl(case: &str, program: &str) -> (PathBuf, Output) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let case_dir = manifest_dir.join("target/compile-cases").join(case);
    let anchor_lang = manifest_dir
        .parent()
        .expect("tests-v2 should live under the workspace root")
        .join("lang-v2");
    let program_dir = case_dir.join("program");
    write_crate(
        &program_dir,
        case,
        &anchor_lang,
        r#"idl-build = ["anchor-lang/idl-build"]"#,
        &format!("{MODELS}{program}"),
    );
    let output = cargo_test(
        &program_dir,
        &case_dir.join("target"),
        &["__anchor_private_print_idl", "--features", "idl-build"],
    );
    (case_dir, output)
}

fn defined(name: &str, generics: Vec<IdlType>) -> IdlType {
    IdlType::Defined {
        name: name.into(),
        generics: generics
            .into_iter()
            .map(|ty| IdlGenericArg::Type { ty })
            .collect(),
    }
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn aliases_reference_the_registered_type() {
    let (case_dir, output) = print_idl("idl_alias_names", PROGRAM);
    assert_success(&case_dir.join("program"), &output);
    let stdout = String::from_utf8(output.stdout).unwrap();

    let mut program: serde_json::Value =
        serde_json::from_str(&idl_block(&stdout, "program")).unwrap();
    let instruction = &program["instructions"][0];
    let args: Vec<IdlType> = instruction["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| serde_json::from_value(arg["type"].clone()).unwrap())
        .collect();
    assert_eq!(
        args,
        [defined("BumpArgs", vec![]), defined("Holder", vec![])]
    );
    let returns: IdlType = serde_json::from_value(instruction["returns"].clone()).unwrap();
    assert_eq!(returns, defined("BumpArgs", vec![]));

    let types: Vec<IdlTypeDef> = serde_json::from_value(program["types"].clone()).unwrap();
    let mut names: Vec<&str> = types.iter().map(|ty| ty.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["BumpArgs", "Holder", "Wrapper"]);

    let holder = types.iter().find(|ty| ty.name == "Holder").unwrap();
    let IdlTypeDefTy::Struct {
        fields: Some(IdlDefinedFields::Named(fields)),
    } = &holder.ty
    else {
        panic!("expected named-field struct, got {:?}", holder.ty);
    };
    let fields: Vec<&IdlType> = fields.iter().map(|field| &field.ty).collect();
    assert_eq!(
        fields,
        [
            &defined("BumpArgs", vec![]),
            &defined("BumpArgs", vec![]),
            &IdlType::Vec(Box::new(defined("BumpArgs", vec![]))),
            &defined("Wrapper", vec![defined("BumpArgs", vec![])]),
        ]
    );

    // The IDL must declare a program whose types are the registered ones
    program["address"] = idl_block(&stdout, "address").trim().into();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let anchor_lang = manifest_dir.parent().unwrap().join("lang-v2");
    let consumer_dir = case_dir.join("consumer");
    write_crate(
        &consumer_dir,
        "idl-alias-names-consumer",
        &anchor_lang,
        "",
        CONSUMER,
    );
    fs::create_dir_all(consumer_dir.join("idls")).unwrap();
    fs::write(
        consumer_dir.join("idls/aliases.json"),
        serde_json::to_string(&program).unwrap(),
    )
    .unwrap();
    let output = cargo_test(&consumer_dir, &case_dir.join("target"), &[]);
    assert_success(&consumer_dir, &output);
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns cargo and writes temporary workspaces; covered by normal cargo test"
)]
fn aliases_of_generic_types_are_rejected() {
    let (_, output) = print_idl("idl_generic_alias", GENERIC_ALIAS_PROGRAM);
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "IDL type `GenericAlias` refers to the generic type `Wrapper` through an alias"
        ),
        "{stdout}"
    );
}
