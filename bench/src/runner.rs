use {
    crate::{
        fixture::Workspace,
        litesvm, markdown,
        results::{Results, VersionResult},
        toolchain::Toolchain,
        version::Version,
    },
    anyhow::{bail, Context, Result},
    std::{
        env,
        fmt::Display,
        fs,
        path::{Path, PathBuf},
        process::{Command, Output, Stdio},
    },
};

pub struct Runner {
    verbose: bool,
    repo: PathBuf,
    bench_dir: PathBuf,
    results: Results,
    current_anchor: String,
    avm: Option<PathBuf>,
}

impl Runner {
    pub fn new(verbose: bool) -> Result<Self> {
        let bench_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo = bench_dir
            .parent()
            .context("bench must be inside the Anchor workspace")?
            .to_owned();
        let results = Results::load(bench_dir.join("results.json"))?;
        let current_anchor = fs::read_to_string(repo.join("VERSION"))?.trim().to_owned();
        Ok(Self {
            verbose,
            repo,
            bench_dir,
            results,
            current_anchor,
            avm: None,
        })
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    pub fn bench_dir(&self) -> &Path {
        &self.bench_dir
    }

    pub fn current_anchor(&self) -> &str {
        &self.current_anchor
    }

    pub fn avm(&self) -> Result<&Path> {
        self.avm
            .as_deref()
            .context("AVM has not been built by the benchmark runner")
    }

    pub fn set_avm(&mut self, avm: PathBuf) {
        self.avm = Some(avm);
    }

    fn sync_markdown(&self, results: &Results) -> Result<()> {
        markdown::sync(&self.repo.join("bench"), results)
    }

    pub fn select_versions(&self, requested: &[String]) -> Result<Vec<Version>> {
        if requested.is_empty() {
            return Ok(vec![Version::Unreleased]);
        }
        if requested.iter().any(|version| version == "all") {
            if requested != ["all"] {
                bail!("'all' cannot be combined with another version");
            }
            let mut selected = vec![Version::Unreleased];
            for version in self.results.versions() {
                let version = version?;
                if !version.is_unreleased() && self.has_lock(&version) {
                    selected.push(version);
                }
            }
            return Ok(selected);
        }

        let mut selected = Vec::new();
        for name in requested {
            let version = Version::parse(name)?;
            if self.results.get(&version).is_none() {
                bail!("Unknown version: {name}");
            }
            if !self.has_lock(&version) {
                bail!("Anchor {name} is a historical result without a benchmark lockfile");
            }
            if !selected.contains(&version) {
                selected.push(version);
            }
        }
        Ok(selected)
    }

    fn has_lock(&self, version: &Version) -> bool {
        self.bench_dir
            .join("locks")
            .join(format!("{version}.lock"))
            .is_file()
    }

    pub fn solana_version(&self, version: &Version) -> Result<&str> {
        self.results
            .get(version)
            .map(|result| result.solana_version.as_str())
            .with_context(|| format!("No benchmark result for Anchor {version}"))
    }

    pub fn run_benchmarks(&mut self, versions: &[Version], check: bool) -> Result<bool> {
        let mut exceeds_threshold = false;

        for (index, version) in versions.iter().enumerate() {
            if index != 0 {
                println!();
            }
            println!("Benchmarking {version}");
            let workspace = Workspace::prepare(self, version)?;
            let tools = self.resolve_toolchain(&workspace)?;
            self.install_toolchain(&workspace, &tools)?;
            let result = self.benchmark(&workspace, &tools)?;
            self.log(result.to_json(version)?);
            let comparison = result.compare(self.results.get(version));
            if comparison.lines.is_empty() {
                println!("No change");
            } else {
                println!("Diff:\n{}", comparison.lines.join("\n"));
            }
            exceeds_threshold |= comparison.exceeds_threshold;
            if !check {
                self.results.update(version, result);
            }
        }

        if !check {
            self.results.save()?;
            self.sync_markdown(&self.results)?;
        }
        Ok(check && exceeds_threshold)
    }

    fn benchmark(&self, workspace: &Workspace, tools: &Toolchain) -> Result<VersionResult> {
        let artifacts = workspace.build(self, tools)?;
        Ok(VersionResult::new(
            tools,
            fs::metadata(&artifacts.deploy)?.len(),
            litesvm::measure(&artifacts.deploy)?,
            self.measure_stack(&tools.platform_tools, &artifacts.stack)?,
        ))
    }

    pub fn run(&self, command: &mut Command) -> Result<()> {
        self.prepare(command);
        self.log(format_args!("+ {command:?}"));
        if self.verbose {
            let status = command
                .status()
                .with_context(|| format!("Failed to run {}", program_name(command)))?;
            if !status.success() {
                bail!("Command failed with {status}: {command:?}");
            }
        } else {
            let output = command
                .output()
                .with_context(|| format!("Failed to run {}", program_name(command)))?;
            check_status(command, &output)?;
        }
        Ok(())
    }

    pub fn output(&self, command: &mut Command) -> Result<String> {
        self.prepare(command);
        self.log(format_args!("+ {command:?}"));
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .with_context(|| format!("Failed to run {}", program_name(command)))?;
        check_status(command, &output)?;
        String::from_utf8(output.stdout).context("Command output was not UTF-8")
    }

    pub fn log(&self, message: impl Display) {
        if self.verbose {
            println!("{message}");
        }
    }

    fn prepare(&self, command: &mut Command) {
        command
            .env("AVM_HOME", self.bench_dir.join(".cache/avm"))
            .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback");
    }
}

fn program_name(command: &Command) -> String {
    command.get_program().to_string_lossy().into_owned()
}

fn check_status(command: &Command, output: &Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!(
        "Command failed with {}: {command:?}\n{stdout}{stderr}",
        output.status
    )
}
