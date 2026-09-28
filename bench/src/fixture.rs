use {
    crate::{runner::Runner, toolchain::Toolchain, version::Version},
    anyhow::{bail, Context, Result},
    semver::Version as Semver,
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

const BENCH_REVISIONS: &[(&str, &str)] = &[("0.29.0", "v1")];
const ESCROW_REVISIONS: &[(&str, &str)] = &[("0.29.0", "v1")];

#[derive(Clone, Copy)]
pub enum Program {
    Bench,
    Escrow,
}

impl Program {
    pub const ALL: &[Self] = &[Self::Bench, Self::Escrow];

    pub fn name(self) -> &'static str {
        match self {
            Self::Bench => "bench",
            Self::Escrow => "escrow",
        }
    }

    pub fn result_name(self, name: &str) -> String {
        match self {
            Self::Bench => name.to_owned(),
            _ => format!("{}/{name}", self.name()),
        }
    }

    pub fn max_init_accounts(self, version: &Version) -> usize {
        match (self, version.as_str()) {
            (Self::Bench, "0.30.0" | "0.30.1") => 4,
            _ => usize::MAX,
        }
    }

    pub fn stack_cases(self) -> Vec<(String, String)> {
        match self {
            Self::Bench => crate::cases::stack_cases(),
            Self::Escrow => [("initialize", "Initialize"), ("take", "Take")]
                .into_iter()
                .map(|(result, name)| (result.into(), name.into()))
                .collect(),
        }
    }

    fn revisions(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Bench => BENCH_REVISIONS,
            Self::Escrow => ESCROW_REVISIONS,
        }
    }

    fn revision(self, version: &Version) -> Result<&'static str> {
        let revisions = self.revisions();
        if version.is_unreleased() {
            return revisions
                .last()
                .map(|(_, revision)| *revision)
                .with_context(|| format!("No revisions configured for {}", self.name()));
        }
        let version = Semver::parse(version.as_str())?;
        revisions
            .iter()
            .rev()
            .find(|(since, _)| Semver::parse(since).is_ok_and(|since| version >= since))
            .map(|(_, revision)| *revision)
            .with_context(|| format!("No {} fixture supports Anchor {version}", self.name()))
    }
}

pub struct Workspace {
    root: PathBuf,
    version: Version,
    programs: Vec<Program>,
}

pub struct Artifacts {
    pub program: Program,
    pub deploy: PathBuf,
    pub stack: PathBuf,
}

impl Workspace {
    pub fn prepare(runner: &Runner, version: &Version) -> Result<Self> {
        let root = runner.bench_dir().join(".work").join(version.as_str());
        clean_workspace(&root)?;
        let programs = Program::ALL.to_vec();
        copy_fixtures(runner, &root, version, &programs)?;

        write_dependencies(runner, &root, version, &programs)?;

        let lock = runner
            .bench_dir()
            .join("locks")
            .join(format!("{version}.lock"));
        if !lock.is_file() {
            bail!("Missing benchmark lockfile {}", lock.display());
        }
        fs::copy(&lock, root.join("Cargo.lock"))?;
        Ok(Self {
            root,
            version: version.clone(),
            programs,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn build(&self, runner: &Runner, tools: &Toolchain) -> Result<Vec<Artifacts>> {
        let help = runner.output(Command::new("cargo-build-sbf").arg("--help"))?;
        let cargo_architecture =
            if tools.sbpf_version == "v0" && help.contains("possible values: sbfv1, sbfv2") {
                "sbfv1"
            } else {
                &tools.sbpf_version
            };
        let target = match cargo_architecture {
            "sbfv1" => "sbf-solana-solana".to_owned(),
            "v0" => "sbpf-solana-solana".to_owned(),
            version => format!("sbpf{version}-solana-solana"),
        };

        let mut artifacts = Vec::new();
        for &program in &self.programs {
            let mut command = Command::new("cargo-build-sbf");
            command
                .arg("--manifest-path")
                .arg(
                    self.root
                        .join("programs")
                        .join(program.name())
                        .join("Cargo.toml"),
                )
                .args([
                    "--tools-version",
                    &tools.platform_tools,
                    "--arch",
                    cargo_architecture,
                    "--",
                    "--locked",
                ])
                .current_dir(&self.root)
                .env(
                    "CARGO",
                    runner
                        .bench_dir()
                        .join(".cache/avm/platform-tools")
                        .join(&tools.platform_tools)
                        .join("rust/bin/cargo"),
                )
                .env("RUSTC_BOOTSTRAP", "1")
                .env(
                    format!(
                        "CARGO_TARGET_{}_RUSTFLAGS",
                        target.to_ascii_uppercase().replace('-', "_")
                    ),
                    "-Zemit-stack-sizes",
                );
            fs::copy(
                runner
                    .bench_dir()
                    .join("locks")
                    .join(format!("{}.lock", self.version)),
                self.root.join("Cargo.lock"),
            )?;
            runner.run(&mut command)?;

            let name = program.name();
            let deploy = [
                self.root.join(format!("target/deploy/{name}.so")),
                self.root.join(format!("deploy/{name}.so")),
            ]
            .into_iter()
            .find(|path| path.is_file())
            .with_context(|| format!("cargo build-sbf did not produce {name}.so"))?;
            let stack = self
                .root
                .join("target")
                .join(&target)
                .join(format!("release/{name}.so"));
            if !stack.is_file() {
                bail!("Could not locate the unstripped {name} SBF artifact");
            }
            artifacts.push(Artifacts {
                program,
                deploy,
                stack,
            });
        }
        Ok(artifacts)
    }
}

pub fn generate_lock(runner: &Runner) -> Result<Vec<u8>> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().join("fixture");
    let programs = Program::ALL.to_vec();
    copy_fixtures(runner, &root, &Version::Unreleased, &programs)?;
    write_dependencies(runner, &root, &Version::Unreleased, &programs)?;
    runner.run(
        Command::new("cargo")
            .args(["generate-lockfile", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .current_dir(&root),
    )?;
    Ok(fs::read(root.join("Cargo.lock"))?)
}

fn copy_fixtures(
    runner: &Runner,
    root: &Path,
    version: &Version,
    programs: &[Program],
) -> Result<()> {
    let fixture = runner.bench_dir().join("fixture");
    fs::create_dir_all(root)?;
    fs::copy(fixture.join("Cargo.toml"), root.join("Cargo.toml"))?;
    for &program in programs {
        copy_dir(
            &fixture
                .join("programs")
                .join(program.name())
                .join(program.revision(version)?),
            &root.join("programs").join(program.name()),
        )?;
    }
    Ok(())
}

fn write_dependencies(
    runner: &Runner,
    root: &Path,
    version: &Version,
    programs: &[Program],
) -> Result<()> {
    for program in programs {
        let manifest = root
            .join("programs")
            .join(program.name())
            .join("Cargo.toml");
        let mut contents = fs::read_to_string(&manifest)?;
        for (name, path) in [("anchor-lang", "lang"), ("anchor-spl", "spl")] {
            let dependency = if version.is_unreleased() {
                format!(
                    "{name} = {{ path = {} }}",
                    serde_json::to_string(
                        &runner.repo().join(path).canonicalize()?.to_string_lossy()
                    )?
                )
            } else if version.as_str() == "1.1.0" {
                format!(
                    "{name} = {{ git = \"https://github.com/otter-sec/anchor\", tag = \
                     \"v{version}\" }}"
                )
            } else {
                format!("{name} = \"={version}\"")
            };
            contents = contents.replace(&format!("{name} = \"=0.0.0\""), &dependency);
        }
        fs::write(&manifest, contents)?;
    }
    Ok(())
}

fn clean_workspace(workspace: &Path) -> Result<()> {
    if !workspace.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(workspace)? {
        let entry = entry?;
        if entry.file_name() == "target" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("Reading fixture directory {}", source.display()))?
    {
        let entry = entry?;
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &destination)?;
        } else {
            fs::copy(source, destination)?;
        }
    }
    Ok(())
}
