use {
    crate::{runner::Runner, toolchain::Toolchain, version::Version},
    anyhow::{bail, Context, Result},
    std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    },
};

pub struct Workspace {
    root: PathBuf,
    version: Version,
}

pub struct Artifacts {
    pub deploy: PathBuf,
    pub stack: PathBuf,
}

impl Workspace {
    pub fn prepare(runner: &Runner, version: &Version) -> Result<Self> {
        let root = runner.bench_dir().join(".work").join(version.as_str());
        clean_workspace(&root)?;
        copy_dir(&runner.bench_dir().join("fixture"), &root)?;

        write_dependencies(runner, &root, version)?;

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
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn build(&self, runner: &Runner, tools: &Toolchain) -> Result<Artifacts> {
        let help = runner.output(Command::new("cargo").args(["build-sbf", "--help"]))?;
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

        let mut command = Command::new("cargo");
        command
            .args(["build-sbf", "--manifest-path"])
            .arg(self.root.join("programs/bench/Cargo.toml"))
            .args([
                "--tools-version",
                &tools.platform_tools,
                "--arch",
                cargo_architecture,
                "--",
                "--locked",
            ])
            .current_dir(&self.root)
            .env("RUSTC_BOOTSTRAP", "1")
            .env(
                format!(
                    "CARGO_TARGET_{}_RUSTFLAGS",
                    target.to_ascii_uppercase().replace('-', "_")
                ),
                "-Zemit-stack-sizes",
            );
        runner.run(&mut command)?;

        let deploy = [
            self.root.join("target/deploy/bench.so"),
            self.root.join("deploy/bench.so"),
        ]
        .into_iter()
        .find(|path| path.is_file())
        .context("cargo build-sbf did not produce target/deploy/bench.so")?;
        let stack = self
            .root
            .join("target")
            .join(target)
            .join("release/bench.so");
        if !stack.is_file() {
            bail!("Could not locate the unstripped SBF artifact");
        }
        Ok(Artifacts { deploy, stack })
    }
}

pub fn generate_lock(runner: &Runner) -> Result<Vec<u8>> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().join("fixture");
    copy_dir(&runner.bench_dir().join("fixture"), &root)?;
    write_dependencies(runner, &root, &Version::Unreleased)?;
    runner.run(
        Command::new("cargo")
            .args(["generate-lockfile", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .current_dir(&root),
    )?;
    Ok(fs::read(root.join("Cargo.lock"))?)
}

fn write_dependencies(runner: &Runner, root: &Path, version: &Version) -> Result<()> {
    let manifest = root.join("programs/bench/Cargo.toml");
    let mut contents = fs::read_to_string(&manifest)?;
    for (name, path) in [("anchor-lang", "lang"), ("anchor-spl", "spl")] {
        let dependency = if version.is_unreleased() {
            format!(
                "{name} = {{ path = {} }}",
                serde_json::to_string(&runner.repo().join(path).canonicalize()?.to_string_lossy())?
            )
        } else if version.as_str() == "1.1.0" {
            format!(
                "{name} = {{ git = \"https://github.com/otter-sec/anchor\", tag = \"v{version}\" \
                 }}"
            )
        } else {
            format!("{name} = \"={version}\"")
        };
        contents = contents.replace(&format!("{name} = \"=0.0.0\""), &dependency);
    }
    fs::write(&manifest, contents)?;
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
