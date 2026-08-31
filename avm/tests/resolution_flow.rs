#![cfg(unix)]

use {
    sha2::{Digest, Sha256},
    std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command, Output},
    },
    tempfile::TempDir,
};

struct Fixture {
    _temp: TempDir,
    avm_home: PathBuf,
    anchor_stub: PathBuf,
    path_bin: PathBuf,
    log_path: PathBuf,
    cargo_log_path: PathBuf,
    rustup_log_path: PathBuf,
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
        let solana_bin = temp
            .path()
            .join("home/.local/share/solana/install/active_release/bin");
        fs::create_dir_all(&solana_bin).expect("solana bin");
        write_executable(
            &solana_bin.join("cargo-build-sbf"),
            r#"#!/bin/sh
echo "$*" >> "$AVM_TEST_CARGO_LOG"
if [ "$1" = "--help" ]; then
  echo "    --install-only"
  exit 0
fi
install_only=
version=
previous=
for argument in "$@"; do
  if [ "$previous" = "--tools-version" ]; then
    version="$argument"
  fi
  if [ "$argument" = "--install-only" ]; then
    install_only=1
  fi
  previous="$argument"
done
if [ -n "$install_only" ] && [ -n "$version" ]; then
  mkdir -p "$HOME/.cache/solana/$version/platform-tools/rust"
  : > "$HOME/.cache/solana/$version/platform-tools/rust/marker"
fi
"#,
        );
        write_executable(
            &path_bin.join("rustup"),
            r#"#!/bin/sh
echo "$*" >> "$AVM_TEST_RUSTUP_LOG"
if [ "$1" = "toolchain" ] && [ "$2" = "list" ]; then
  echo "nightly-2025-04-15-x86_64-unknown-linux-gnu"
  echo "nightly-2026-06-10-x86_64-unknown-linux-gnu"
  exit 0
fi
if [ "$1" = "toolchain" ] && [ "$2" = "link" ]; then
  exit 0
fi
if [ "$1" = "toolchain" ] && [ "$2" = "uninstall" ]; then
  exit 0
fi
if [ "$1" = "toolchain" ] && [ "$2" = "install" ]; then
  exit 0
fi
exit 1
"#,
        );

        Self {
            _temp: temp,
            avm_home,
            anchor_stub,
            path_bin,
            log_path: avm_bin.join("anchor.log"),
            cargo_log_path: avm_bin.join("cargo.log"),
            rustup_log_path: avm_bin.join("rustup.log"),
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
echo "rustup_toolchain=${{RUSTUP_TOOLCHAIN:-}}" >> "$AVM_TEST_ANCHOR_LOG"
echo "resolver=${{CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS:-}}" >> "$AVM_TEST_ANCHOR_LOG"
echo "next_lockfile_bump=${{CARGO_UNSTABLE_NEXT_LOCKFILE_BUMP:-}}" >> "$AVM_TEST_ANCHOR_LOG"
"#
            ),
        );
    }

    fn install_anchor_with_cargo_calls(&self, version: &str) {
        write_executable(
            &self.avm_home_bin().join(format!("anchor-{version}")),
            r#"#!/bin/sh
cargo build-sbf
cargo +nightly test idl
cargo +nightly-2026-07-01 test already-pinned
"#,
        );
        write_executable(
            &self.path_bin.join("cargo"),
            r#"#!/bin/sh
echo "$*" >> "$AVM_TEST_CARGO_LOG"
install_only=
version=
previous=
for argument in "$@"; do
  if [ "$previous" = "--tools-version" ]; then
    version="$argument"
  fi
  if [ "$argument" = "--install-only" ]; then
    install_only=1
  fi
  previous="$argument"
done
if [ -n "$install_only" ] && [ -n "$version" ]; then
  mkdir -p "$HOME/.cache/solana/$version/platform-tools/rust"
  : > "$HOME/.cache/solana/$version/platform-tools/rust/marker"
fi
"#,
        );
    }

    fn install_legacy_build_sbf(&self) {
        let build_sbf = self
            ._temp
            .path()
            .join("home/.local/share/solana/install/active_release/bin/cargo-build-sbf");
        write_executable(
            &build_sbf,
            r#"#!/bin/sh
echo "$*" >> "$AVM_TEST_CARGO_LOG"
if [ "$1" = "--help" ]; then
  echo "legacy cargo-build-sbf"
  exit 0
fi
exit 1
"#,
        );
    }

    fn cache_platform_tools(&self, version: &str) -> PathBuf {
        let path = self
            ._temp
            .path()
            .join("home/.cache/solana")
            .join(version)
            .join("platform-tools");
        fs::create_dir_all(path.join("rust")).expect("platform-tools rust dir");
        fs::write(path.join("rust/marker"), "cached").expect("platform-tools marker");
        path
    }

    fn install_nightly_anchor_via_script(&self) {
        let installer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("install");
        assert!(
            installer.is_file(),
            "expected checkout installer at {}",
            installer.display()
        );

        let nightly_dir = self._temp.path().join("nightly");
        let avm_src = nightly_dir.join("avm-src");
        let anchor_src = nightly_dir.join("anchor-src");
        fs::create_dir_all(&avm_src).expect("avm src");
        fs::create_dir_all(&anchor_src).expect("anchor src");

        write_executable(
            &avm_src.join("avm"),
            r#"#!/bin/sh
echo "fake nightly avm"
"#,
        );
        write_executable(
            &anchor_src.join("anchor"),
            r#"#!/bin/sh
echo "version=nightly" > "$AVM_TEST_ANCHOR_LOG"
echo "args=$*" >> "$AVM_TEST_ANCHOR_LOG"
"#,
        );

        let avm_archive = nightly_dir.join("avm.tar.gz");
        let anchor_archive = nightly_dir.join("anchor.tar.gz");
        create_tar_gz(&avm_archive, &avm_src, "avm");
        create_tar_gz(&anchor_archive, &anchor_src, "anchor");

        let manifest = nightly_dir.join("manifest.json");
        fs::write(
            &manifest,
            format!(
                r#"{{
  "version": "checkout-nightly-test",
  "artifacts": [
    {{
      "tool": "avm",
      "target": "{}",
      "file": "avm.tar.gz",
      "s3_key": "avm.tar.gz",
      "sha256": "{}"
    }},
    {{
      "tool": "anchor",
      "target": "{}",
      "file": "anchor.tar.gz",
      "s3_key": "anchor.tar.gz",
      "sha256": "{}"
    }}
  ]
}}
"#,
                nightly_target(),
                sha256_file(&avm_archive),
                nightly_target(),
                sha256_file(&anchor_archive)
            ),
        )
        .expect("manifest");

        let cargo_home = self._temp.path().join("cargo-home");
        let home = self._temp.path().join("home");
        fs::create_dir_all(&home).expect("home");

        write_executable(
            &self.avm_home_bin().join("avm"),
            r#"#!/bin/sh
echo "fake stable avm"
"#,
        );

        let output = Command::new("sh")
            .arg(&installer)
            .env("AVM_HOME", &self.avm_home)
            .env("CARGO_HOME", &cargo_home)
            .env("HOME", &home)
            .env("AVM_INSTALL_TARGET", nightly_target())
            .env(
                "AVM_NIGHTLY_MANIFEST_URL",
                format!("file://{}", manifest.display()),
            )
            .env(
                "AVM_NIGHTLY_BASE_URL",
                format!("file://{}/", nightly_dir.display()),
            )
            .output()
            .expect("run checkout installer");
        assert_success(&output);

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Installed Anchor nightly checkout-nightly-test"),
            "{stdout}"
        );
        assert!(
            stdout.contains("Add this to your shell profile if avm is not already on PATH:"),
            "{stdout}"
        );
        assert!(
            stdout.contains(&format!(
                "export PATH=\"{}:$PATH\"",
                self.avm_home_bin().display()
            )),
            "{stdout}"
        );
        assert!(
            self.avm_home_bin().join("avm-nightly").is_file(),
            "avm-nightly should be installed"
        );
        assert!(
            self.avm_home_bin().join("anchor-nightly").is_file(),
            "anchor-nightly should be installed"
        );
        assert!(
            self.avm_home_bin().join("avm-stable").is_file(),
            "stable AVM should be backed up"
        );
        assert_eq!(
            fs::read_to_string(self.avm_home.join(".nightly")).expect(".nightly"),
            "enabled\n"
        );
        assert!(fs::read_to_string(self.avm_home.join(".nightly-check"))
            .expect(".nightly-check")
            .contains("checkout-nightly-test"));
        assert!(
            !cargo_home.join("bin").exists(),
            "missing CARGO_HOME/bin should be a no-op, not an early exit"
        );
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
            .env("HOME", self._temp.path().join("home"))
            .env("PATH", path)
            .env_remove("RUSTUP_TOOLCHAIN")
            .env_remove("CARGO_UNSTABLE_NEXT_LOCKFILE_BUMP")
            .env("AVM_TEST_ANCHOR_LOG", &self.log_path)
            .env("AVM_TEST_CARGO_LOG", &self.cargo_log_path)
            .env("AVM_TEST_RUSTUP_LOG", &self.rustup_log_path)
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
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/lib.rs"), "").unwrap();

    assert_success(&fixture.run_anchor(&project, ["build", "--", "--features", "mainnet"]));

    let log = fixture.anchor_log();
    assert!(log.contains("version=1.0.2"), "{log}");
    assert!(log.contains("args=build -- --features mainnet"), "{log}");
    assert!(log.contains("avm_active=1"), "{log}");
    assert!(log.contains("rustup_toolchain=\n"), "{log}");
    assert!(log.contains("resolver=fallback"), "{log}");
    assert_eq!(
        fs::read_to_string(&fixture.solana_log_path).unwrap(),
        "--version\n"
    );
    assert_eq!(
        fs::read_to_string(&fixture.cargo_log_path).unwrap(),
        "--help\nbuild-sbf --install-only --tools-version v1.52\n"
    );
    let rustup_log = fs::read_to_string(&fixture.rustup_log_path).unwrap();
    assert!(rustup_log.contains("toolchain list -v\n"), "{rustup_log}");
    assert!(
        rustup_log.contains("toolchain link 1.89.0-sbpf-solana-v1.52"),
        "{rustup_log}"
    );
}

#[test]
fn anchor_stub_falls_back_to_anchorversion_cargo_and_global_sources() {
    let fixture = Fixture::new();
    fixture.install_legacy_build_sbf();
    fixture.cache_platform_tools("v1.48");
    fixture.cache_platform_tools("v1.43");
    fixture.cache_platform_tools("v1.41");
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
    fs::create_dir_all(cargo_project.join("src")).unwrap();
    fs::write(cargo_project.join("src/lib.rs"), "").unwrap();
    assert_success(&fixture.run_anchor(&cargo_project, ["idl", "build"]));
    assert!(fixture.anchor_log().contains("version=0.31.1"));

    let global_project = fixture.project("global");
    fixture.install_fake_solana("1.18.17");
    assert_success(&fixture.run_anchor(&global_project, ["--version"]));
    assert!(fixture.anchor_log().contains("version=0.30.1"));
    assert!(
        !fixture.cargo_log_path.exists(),
        "a cached legacy toolchain must not invoke unsupported --install-only"
    );
    let rustup_log = fs::read_to_string(&fixture.rustup_log_path).unwrap();
    assert_eq!(rustup_log.matches("toolchain link solana ").count(), 3);
}

#[test]
fn anchor_stub_pins_only_unversioned_nightly_cargo_invocations() {
    let fixture = Fixture::new();
    let project = fixture.project("cargo-proxy");
    fixture.install_anchor_with_cargo_calls("1.0.2");
    fixture.install_fake_solana("4.1.2");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"1.0.2\"\nsolana_version = \"4.1.2\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["build"]));

    assert_eq!(
        fs::read_to_string(&fixture.cargo_log_path).unwrap(),
        "--help\nbuild-sbf --install-only --tools-version v1.54\nbuild-sbf\n+nightly-2026-06-10 \
         test idl\n+nightly-2026-07-01 test already-pinned\n"
    );
    let rustup_log = fs::read_to_string(&fixture.rustup_log_path).unwrap();
    assert!(
        rustup_log.contains("toolchain link 1.89.0-sbpf-solana-v1.54"),
        "{rustup_log}"
    );
}

#[test]
fn anchor_stub_uses_legacy_idl_nightly_for_locked_proc_macro2() {
    let fixture = Fixture::new();
    fixture.install_legacy_build_sbf();
    fixture.cache_platform_tools("v1.41");
    let project = fixture.project("legacy-cargo-proxy");
    fixture.install_anchor_with_cargo_calls("0.30.1");
    fixture.install_fake_solana("1.18.17");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"0.30.1\"\nsolana_version = \"1.18.17\"\n",
    )
    .unwrap();
    fs::write(
        project.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"proc-macro2\"\nversion = \"1.0.86\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["build"]));

    assert_eq!(
        fs::read_to_string(&fixture.cargo_log_path).unwrap(),
        "build-sbf\n+nightly-2025-04-15 test idl\n+nightly-2026-07-01 test already-pinned\n"
    );
    let rustup_log = fs::read_to_string(&fixture.rustup_log_path).unwrap();
    assert!(
        rustup_log.contains("toolchain link solana "),
        "{rustup_log}"
    );
}

#[test]
fn anchor_stub_enables_v4_lockfile_for_compatible_legacy_cargo() {
    let fixture = Fixture::new();
    fixture.install_legacy_build_sbf();
    fixture.cache_platform_tools("v1.41");
    fixture.install_anchor("0.30.1");
    fixture.install_fake_solana("1.18.17");
    let project = fixture.project("legacy-v4-lockfile");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"0.30.1\"\nsolana_version = \"1.18.17\"\n",
    )
    .unwrap();
    fs::write(project.join("Cargo.lock"), "version = 4\n").unwrap();

    assert_success(&fixture.run_anchor(&project, ["build"]));

    let log = fixture.anchor_log();
    assert!(log.contains("next_lockfile_bump=true"), "{log}");
}

#[test]
fn anchor_stub_does_not_enable_v4_opt_in_for_native_cargo() {
    let fixture = Fixture::new();
    fixture.install_legacy_build_sbf();
    fixture.cache_platform_tools("v1.43");
    fixture.install_anchor("0.31.1");
    fixture.install_fake_solana("2.1.0");
    let project = fixture.project("native-v4-lockfile");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"0.31.1\"\nsolana_version = \"2.1.0\"\n",
    )
    .unwrap();
    fs::write(project.join("Cargo.lock"), "version = 4\n").unwrap();

    assert_success(&fixture.run_anchor(&project, ["build"]));

    let log = fixture.anchor_log();
    assert!(log.contains("next_lockfile_bump=\n"), "{log}");
}

#[test]
fn anchor_stub_uses_versioned_link_for_early_agave_three() {
    let fixture = Fixture::new();
    fixture.install_legacy_build_sbf();
    fixture.cache_platform_tools("v1.51");
    fixture.install_anchor("1.0.2");
    fixture.install_fake_solana("3.0.0");
    let project = fixture.project("early-agave-three");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"1.0.2\"\nsolana_version = \"3.0.0\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["--version"]));

    let rustup_log = fs::read_to_string(&fixture.rustup_log_path).unwrap();
    assert!(
        rustup_log.contains("toolchain link 1.84.1-sbpf-solana-v1.51"),
        "{rustup_log}"
    );
    assert!(
        !fixture.cargo_log_path.exists(),
        "a cached early 3.x toolchain must not invoke unsupported --install-only"
    );
}

#[test]
fn anchor_stub_supports_oldest_anchor_solana_mapping() {
    let fixture = Fixture::new();
    fixture.install_legacy_build_sbf();
    fixture.cache_platform_tools("v1.37");
    fixture.install_anchor("0.29.0");
    fixture.install_fake_solana("1.17.25");
    let project = fixture.project("anchor-029");
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"0.29.0\"\nsolana_version = \"1.17.25\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["--version"]));

    let rustup_log = fs::read_to_string(&fixture.rustup_log_path).unwrap();
    assert!(
        rustup_log.contains("toolchain link solana "),
        "{rustup_log}"
    );
    assert!(rustup_log.contains("/v1.37/platform-tools/rust"));
    assert!(
        !fixture.cargo_log_path.exists(),
        "Solana 1.17 must not invoke unsupported --install-only"
    );
}

#[test]
fn nightly_stub_takes_precedence_after_installer_bootstrap() {
    let fixture = Fixture::new();
    let project = fixture.project("nightly");
    fixture.install_anchor("1.0.2");
    fixture.install_fake_solana("3.1.10");
    fixture.install_nightly_anchor_via_script();
    fs::write(
        project.join("Anchor.toml"),
        "[toolchain]\nanchor_version = \"1.0.2\"\nsolana_version = \"3.1.10\"\n",
    )
    .unwrap();

    assert_success(&fixture.run_anchor(&project, ["build"]));

    let log = fixture.anchor_log();
    assert!(log.contains("version=nightly"), "{log}");
    assert!(log.contains("args=build"), "{log}");
    assert_eq!(
        fs::read_to_string(&fixture.solana_log_path).unwrap(),
        "--version\n"
    );
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

fn create_tar_gz(archive: &Path, source_dir: &Path, entry: &str) {
    let output = Command::new("tar")
        .arg("-czf")
        .arg(archive)
        .arg("-C")
        .arg(source_dir)
        .arg(entry)
        .output()
        .expect("create tar.gz");
    assert_success(&output);
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read archive");
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn nightly_target() -> &'static str {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        (os, arch) => panic!("unsupported test target {os}/{arch}"),
    }
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
