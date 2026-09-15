use {anyhow::Result, semver::Version as Semver, std::fmt};

#[derive(Clone, PartialEq, Eq)]
pub enum Version {
    Released(String),
    Unreleased,
}

impl Version {
    pub fn parse(version: &str) -> Result<Self> {
        if version == "unreleased" {
            Ok(Self::Unreleased)
        } else {
            Semver::parse(version)?;
            Ok(Self::Released(version.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Released(version) => version,
            Self::Unreleased => "unreleased",
        }
    }

    pub fn is_unreleased(&self) -> bool {
        matches!(self, Self::Unreleased)
    }

    pub fn sbpf_version(&self) -> Result<&'static str> {
        Ok(match self {
            Self::Released(version) if Semver::parse(version)? < Semver::new(1, 2, 0) => "v0",
            _ => "v3",
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
