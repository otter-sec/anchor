#![cfg(unix)]

use {
    std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Output},
        time::{SystemTime, UNIX_EPOCH},
    },
    tempfile::TempDir,
};

struct Fixture {
    _temp: TempDir,
    avm_home: PathBuf,
    anchor_stub: PathBuf,
    path_bin: PathBuf,
    log_path: PathBuf,
    solana_log_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let avm_home = temp.path().join("avm-home");
        let avm_bin = avm_home.join("bin");
        fs::create_dir_all(&avm_bin).expect("avm bin");

        let path_bin = temp.path().join("path-bin");
        fs::create_dir_all(&path_bin).expect("path bin");
        let anchor_stub = path_bin.join("anchor");
        fs::copy(env!("CARGO_BIN_EXE_avm"), &anchor_stub).expect("copy avm as anchor");
        make_executable(&anchor_stub);

        Self {
            _temp: temp,
            avm_home,
            anchor_stub,
            path_bin,
            log_path: avm_bin.join("anchor.log"),
            solana_log_path: avm_bin.join("solana.log"),
        }
    }

    fn avm_home_bin(&self) -> PathBuf {
        self.avm_home.join("bin")
    }

    fn project(&self, name: &str) -> PathBuf {
        let project = self._temp.path().join(name);
        fs::create_dir_all(&project).expect("project dir");
        project
    }

    fn write_avm_version(&self, version: &str) {
        fs::write(self.avm_home.join(".version"), version).expect(".version");
    }

    fn install_anchor(&self, version: &str) {
        write_executable(
            &self.avm_home_bin().join(format!("anchor-{version}")),
            &format!(
                r#"#!/bin/sh
echo "version={version}" > "$AVM_TEST_ANCHOR_LOG"
echo "args=$*" >> "$AVM_TEST_ANCHOR_LOG"
echo "avm_active=${{AVM_ACTIVE:-}}" >> "$AVM_TEST_ANCHOR_LOG"
echo "resolver=${{CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS:-}}" >> "$AVM_TEST_ANCHOR_LOG"
"#
            ),
        );
    }

    fn install_nightly_anchor(&self) {
        write_executable(
            &self.avm_home_bin().join("anchor-nightly"),
            r#"#!/bin/sh
echo "version=nightly" > "$AVM_TEST_ANCHOR_LOG"
echo "args=$*" >> "$AVM_TEST_ANCHOR_LOG"
"#,
        );
        write_executable(
            &self.avm_home_bin().join("avm-nightly"),
            r#"#!/bin/sh
echo "fake nightly avm"
"#,
        );
        fs::write(self.avm_home.join(".nightly"), "enabled\n").expect(".nightly");
        fs::write(
            self.avm_home.join(".nightly-check"),
            format!("{}\nnightly-test\n", unix_timestamp()),
        )
        .expect(".nightly-check");
    }

    fn install_fake_solana(&self, version: &str) {
        write_executable(
            &self.path_bin.join("solana"),
            &format!(
                r#"#!/bin/sh
echo "$*" >> "$AVM_TEST_SOLANA_LOG"
if [ "$1" = "--version" ]; then
  echo "solana-cli {version}"
  exit 0
fi
exit 0
"#
            ),
        );
    }

    fn run_anchor<I, S>(&self, current_dir: &Path, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let path = format!(
            "{}:{}",
            self.path_bin.display(),
            env::var("PATH").unwrap_or_default()
        );
        Command::new(&self.anchor_stub)
            .args(args)
            .current_dir(current_dir)
            .env("AVM_HOME", &self.avm_home)
            .env("PATH", path)
            .env("AVM_TEST_ANCHOR_LOG", &self.log_path)
            .env("AVM_TEST_SOLANA_LOG", &self.solana_log_path)
            .output()
            .expect("run anchor stub")
    }

    fn run_avm<I, S>(&self, current_dir: &Path, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let path = format!(
            "{}:{}",
            self.path_bin.display(),
            env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_avm"))
            .args(args)
            .current_dir(current_dir)
            .env("AVM_HOME", &self.avm_home)
            .env("PATH", path)
            .output()
            .expect("run avm")
    }

    fn anchor_log(&self) -> String {
        fs::read_to_string(&self.log_path).expect("anchor log")
    }
}

#[test]
fn anchor_stub_prefers_anchor_toml_and_sets_launcher_env() {
    let fixture = Fixture::new();
    let project = fixture.project("anchor-toml");
    fixture.install_anchor("1.0.2");
    fixture.install_anchor("0.32.1");
    fixture.install_anchor("0.31.1");
    fixture.install_anchor("0.30.1");
    fixture.install_fake_solana("3.1.10");
    fixture.write_avm_version("0.30.1");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"1.0.2\"\nsolana_version = \"3.1.10\"\n",
    )
    .unwrap();
    fs::write(project.join(".anchorversion"), "0.32.1\n").unwrap();
    fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"program\"\nversion = \"0.1.0\"\nedition = \
         \"2021\"\n[dependencies]\nanchor-lang = \"0.31.1\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["build", "--", "--features", "mainnet"]));

    let log = fixture.anchor_log();
    assert!(log.contains("version=1.0.2"), "{log}");
    assert!(log.contains("args=build -- --features mainnet"), "{log}");
    assert!(log.contains("avm_active=1"), "{log}");
    assert!(log.contains("resolver=fallback"), "{log}");
    assert_eq!(
        fs::read_to_string(&fixture.solana_log_path).unwrap(),
        "--version\n"
    );
}

#[test]
fn anchor_stub_falls_back_to_anchorversion_cargo_and_global_sources() {
    let fixture = Fixture::new();
    fixture.install_anchor("0.32.1");
    fixture.install_anchor("0.31.1");
    fixture.install_anchor("0.30.1");
    fixture.write_avm_version("0.30.1");

    let anchorversion_project = fixture.project("anchorversion");
    fixture.install_fake_solana("2.3.0");
    fs::write(anchorversion_project.join(".anchorversion"), "0.32.1\n").unwrap();
    assert_success(&fixture.run_anchor(&anchorversion_project, ["keys", "list"]));
    assert!(fixture.anchor_log().contains("version=0.32.1"));

    let cargo_project = fixture.project("cargo");
    fixture.install_fake_solana("2.1.0");
    fs::write(
        cargo_project.join("Cargo.toml"),
        "[package]\nname = \"program\"\nversion = \"0.1.0\"\nedition = \
         \"2021\"\n[dependencies]\nanchor-lang = \"0.31.1\"\n",
    )
    .unwrap();
    assert_success(&fixture.run_anchor(&cargo_project, ["idl", "build"]));
    assert!(fixture.anchor_log().contains("version=0.31.1"));

    let global_project = fixture.project("global");
    fixture.install_fake_solana("1.18.17");
    assert_success(&fixture.run_anchor(&global_project, ["--version"]));
    assert!(fixture.anchor_log().contains("version=0.30.1"));
}

#[test]
fn nightly_stub_takes_precedence_without_network() {
    let fixture = Fixture::new();
    let project = fixture.project("nightly");
    fixture.install_anchor("1.0.2");
    fixture.install_nightly_anchor();
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"1.0.2\"\nsolana_version = \"3.1.10\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["build"]));

    let log = fixture.anchor_log();
    assert!(log.contains("version=nightly"), "{log}");
    assert!(log.contains("args=build"), "{log}");
    assert!(!fixture.solana_log_path.exists());
}

#[test]
fn avm_subcommands_resolve_solana_and_platform_tools_from_project() {
    let fixture = Fixture::new();
    let project = fixture.project("toolchain");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"1.0.2\"\nsolana_version = \"2.3.0\"\n",
    )
    .unwrap();

    let solana = fixture.run_avm(&project, ["solana", "resolve"]);
    assert_success(&solana);
    let solana_stdout = command_stdout(solana);
    assert!(solana_stdout.contains("solana 2.3.0"), "{solana_stdout}");
    assert!(
        solana_stdout.contains("[toolchain] solana_version"),
        "{solana_stdout}"
    );

    let platform_tools = fixture.run_avm(&project, ["platform-tools", "resolve"]);
    assert_success(&platform_tools);
    let platform_tools_stdout = command_stdout(platform_tools);
    assert!(
        platform_tools_stdout.contains("platform-tools v1.48"),
        "{platform_tools_stdout}"
    );
    assert!(
        platform_tools_stdout.contains("solana 2.3.0"),
        "{platform_tools_stdout}"
    );
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write executable");
    make_executable(path);
}

fn make_executable(path: &Path) {
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_secs()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn command_stdout(output: Output) -> String {
    assert_success(&output);
    String::from_utf8(output.stdout).expect("utf8 stdout")
}
