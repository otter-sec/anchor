use {
    crate::{fixture::Workspace, runner::Runner},
    anyhow::{bail, Result},
    regex::Regex,
    std::{env, path::PathBuf, process::Command},
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
        let platform_tools = parse_platform_tools_resolution(
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
        )?;
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
        )
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
