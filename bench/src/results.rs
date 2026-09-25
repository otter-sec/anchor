use {
    crate::{toolchain::Toolchain, version::Version},
    anyhow::{bail, Context, Result},
    indexmap::IndexMap,
    serde::{Deserialize, Serialize},
    std::{fs, io::Write, path::PathBuf},
};

pub struct Results {
    path: PathBuf,
    entries: IndexMap<String, VersionResult>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionResult {
    pub solana_version: String,
    pub platform_tools_version: String,
    #[serde(default)]
    pub sbpf_version: String,
    pub result: Measurements,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurements {
    pub binary_size: IndexMap<String, u64>,
    pub compute_units: IndexMap<String, u64>,
    pub stack_memory: IndexMap<String, u64>,
}

impl Results {
    pub fn load(path: PathBuf) -> Result<Self> {
        let mut entries: IndexMap<String, VersionResult> =
            serde_json::from_slice(&fs::read(&path)?)?;
        for (version, result) in &mut entries {
            if result.sbpf_version.is_empty() {
                result.sbpf_version = Version::parse(version)?.sbpf_version()?.to_owned();
            }
        }
        Ok(Self { path, entries })
    }

    pub fn get(&self, version: &Version) -> Option<&VersionResult> {
        self.entries.get(version.as_str())
    }

    pub fn versions(&self) -> impl Iterator<Item = Result<Version>> + '_ {
        self.entries.keys().map(|version| Version::parse(version))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &VersionResult)> {
        self.entries
            .iter()
            .map(|(version, result)| (version.as_str(), result))
    }

    pub fn update(&mut self, version: &Version, result: VersionResult) {
        self.entries.insert(version.to_string(), result);
    }

    pub fn bump_version(&mut self, version: &Version) -> Result<()> {
        if version.is_unreleased() {
            bail!("Cannot create an unreleased release");
        }
        if self.entries.contains_key(version.as_str()) {
            bail!("Benchmark results already contain Anchor {version}");
        }

        let unreleased = self
            .entries
            .shift_remove("unreleased")
            .context("Benchmark results do not contain an unreleased entry")?;
        self.entries.insert(version.to_string(), unreleased.clone());
        self.entries.insert("unreleased".into(), unreleased);
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let parent = self.path.parent().context("Results path has no parent")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temporary, &self.entries)?;
        temporary.write_all(b"\n")?;
        temporary.persist(&self.path)?;
        Ok(())
    }
}

impl VersionResult {
    pub fn new(
        tools: &Toolchain,
        binary_size: u64,
        compute_units: IndexMap<String, u64>,
        stack_memory: IndexMap<String, u64>,
    ) -> Self {
        Self {
            solana_version: tools.solana.clone(),
            platform_tools_version: tools.platform_tools.clone(),
            sbpf_version: tools.sbpf_version.clone(),
            result: Measurements {
                binary_size: IndexMap::from([("bench".into(), binary_size)]),
                compute_units,
                stack_memory,
            },
        }
    }

    pub fn to_json(&self, version: &Version) -> Result<String> {
        Ok(serde_json::to_string_pretty(&IndexMap::from([(
            version.as_str(),
            self,
        )]))?)
    }
}
