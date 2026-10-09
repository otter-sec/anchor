use {
    crate::{fixture::Workspace, runner::Runner},
    anyhow::{bail, Context, Result},
    regex::Regex,
    std::{env, fs, path::PathBuf, process::Command},
};

pub struct Toolchain {
    pub solana: String,
    pub platform_tools: String,
    pub sbpf_version: String,
}

impl Runner {
    pub fn build_avm(&mut self) -> Result<()> {
        if let Some(path) = env::var_os("BENCH_AVM") {
            self.set_avm(path.into());
            return Ok(());
        }

        self.run(
            Command::new("cargo")
                .args(["build", "--locked", "-p", "avm", "--bin", "avm"])
                .current_dir(self.repo()),
        )?;
        let mut target = env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.repo().join("target"));
        if target.is_relative() {
            target = self.repo().join(target);
        }
        self.set_avm(
            target
                .join("debug")
                .join(format!("avm{}", env::consts::EXE_SUFFIX)),
        );
        Ok(())
    }

    pub fn resolve_toolchain(&self, workspace: &Workspace) -> Result<Toolchain> {
        let solana = self.solana_version(workspace.version())?.to_owned();
        let platform_tools = if workspace.version().is_unreleased() {
            parse_platform_tools_resolution(
                &self.output(
                    Command::new(self.avm()?)
                        .args([
                            "platform-tools",
                            "resolve",
                            "--solana-version",
                            &solana,
                            "--output",
                            "version",
                        ])
                        .current_dir(workspace.root()),
                )?,
            )?
        } else {
            self.platform_tools_version(workspace.version())?.to_owned()
        };
        Ok(Toolchain {
            solana,
            platform_tools,
            sbpf_version: workspace.version().sbpf_version()?.to_owned(),
        })
    }

    pub fn install_toolchain(&self, workspace: &Workspace, tools: &Toolchain) -> Result<()> {
        self.run(
            Command::new(self.avm()?)
                .args(["solana", "install", &tools.solana])
                .current_dir(workspace.root()),
        )?;
        self.run(
            Command::new(self.avm()?)
                .args(["platform-tools", "install", &tools.platform_tools])
                .current_dir(workspace.root()),
        )?;
        self.link_platform_tools(&tools.platform_tools)
    }

    fn link_platform_tools(&self, version: &str) -> Result<()> {
        let source = self
            .bench_dir()
            .join(".cache/avm/platform-tools")
            .join(version);
        if !source.is_dir() {
            bail!(
                "AVM did not install platform-tools {version} at {}",
                source.display()
            );
        }
        let home = env::var_os("HOME").context("HOME is not set")?;
        let cache = PathBuf::from(home).join(".cache/solana").join(version);
        let destination = cache.join("platform-tools");
        if destination.exists() {
            return Ok(());
        }

        fs::create_dir_all(cache)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &destination)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&source, &destination)?;
        Ok(())
    }

    pub fn active_solana(&self) -> Option<String> {
        let output = self.output(Command::new("solana").arg("--version")).ok()?;
        Regex::new(r"solana-cli\s+(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)")
            .unwrap()
            .captures(&output)
            .map(|captures| captures[1].to_owned())
    }

    pub fn restore_solana(&self, previous: Option<&str>) -> Result<()> {
        let Some(previous) = previous else {
            return Ok(());
        };
        if self.active_solana().as_deref() != Some(previous) {
            self.log(format_args!("\n==> Restoring Solana {previous}"));
            self.run(Command::new(self.avm()?).args(["solana", "install", previous]))?;
        }
        Ok(())
    }
}

fn parse_platform_tools_resolution(output: &str) -> Result<String> {
    let version = output.trim();
    if !Regex::new(r"^v\d+\.\d+(?:\.\d+)?$")?.is_match(version) {
        bail!("AVM returned an invalid platform-tools version: {version:?}");
    }
    Ok(version.to_owned())
}
