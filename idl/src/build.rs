use {
    crate::types::{Idl, IdlEvent, IdlTypeDef},
    anyhow::{anyhow, Result},
    regex::Regex,
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        env, mem,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    },
};

/// A trait that types must implement in order to include the type in the IDL definition.
///
/// This trait is automatically implemented for Anchor all types that use the `AnchorSerialize`
/// proc macro. Note that manually implementing the `AnchorSerialize` trait does **NOT** have the
/// same effect.
///
/// Types that don't implement this trait will cause a compile error during the IDL generation.
///
/// The default implementation of the trait allows the program to compile but the type does **NOT**
/// get included in the IDL.
pub trait IdlBuild {
    /// Create an IDL type definition for the type.
    ///
    /// The type is only included in the IDL if this method returns `Some`.
    fn create_type() -> Option<IdlTypeDef> {
        None
    }

    /// Insert all types that are included in the current type definition to the given map.
    fn insert_types(_types: &mut BTreeMap<String, IdlTypeDef>) {}

    /// Get the full module path of the type.
    ///
    /// The full path will be used in the case of a conflicting type definition, e.g. when there
    /// are multiple structs with the same name.
    ///
    /// The default implementation covers most cases.
    fn get_full_path() -> String {
        std::any::type_name::<Self>().into()
    }
}

/// IDL builder using builder pattern.
///
/// # Example
///
/// ```rust,no_run
/// # use std::path::PathBuf;
/// # use anchor_lang_idl::build::IdlBuilder;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let path = PathBuf::from("programs/my_program");
/// let idl = IdlBuilder::new().program_path(path).skip_lint(true).build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct IdlBuilder {
    program_path: Option<PathBuf>,
    resolution: Option<bool>,
    skip_lint: Option<bool>,
    no_docs: Option<bool>,
    cargo_args: Option<Vec<String>>,
}

impl IdlBuilder {
    /// Create a new [`IdlBuilder`] instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the program path (default: current directory)
    pub fn program_path(mut self, program_path: PathBuf) -> Self {
        self.program_path.replace(program_path);
        self
    }

    /// Set whether to include account resolution information in the IDL (default: true).
    pub fn resolution(mut self, resolution: bool) -> Self {
        self.resolution.replace(resolution);
        self
    }
    /// Set whether to skip linting (default: false).
    pub fn skip_lint(mut self, skip_lint: bool) -> Self {
        self.skip_lint.replace(skip_lint);
        self
    }

    /// Set whether to skip generating docs in the IDL (default: false).
    pub fn no_docs(mut self, no_docs: bool) -> Self {
        self.no_docs.replace(no_docs);
        self
    }

    /// Set the `cargo` args that will get passed to the underlying `cargo` command when building
    /// IDLs (default: empty).
    pub fn cargo_args(mut self, cargo_args: Vec<String>) -> Self {
        self.cargo_args.replace(cargo_args);
        self
    }

    /// Build the IDL with the current configuration.
    pub fn build(self) -> Result<Idl> {
        let idl = build(
            &self
                .program_path
                .unwrap_or_else(|| std::env::current_dir().expect("Failed to get program path")),
            self.resolution.unwrap_or(true),
            self.skip_lint.unwrap_or_default(),
            self.no_docs.unwrap_or_default(),
            &self.cargo_args.unwrap_or_default(),
        )
        .map(convert_module_paths)
        .map(sort)?;
        verify(&idl)?;

        Ok(idl)
    }
}

/// Generate IDL via compilation.
#[deprecated(since = "0.1.2", note = "Use `IdlBuilder` instead")]
pub fn build_idl(
    program_path: impl AsRef<Path>,
    resolution: bool,
    skip_lint: bool,
    no_docs: bool,
) -> Result<Idl> {
    IdlBuilder::new()
        .program_path(program_path.as_ref().into())
        .resolution(resolution)
        .skip_lint(skip_lint)
        .no_docs(no_docs)
        .build()
}

/// Build IDL.
fn build(
    program_path: &Path,
    resolution: bool,
    skip_lint: bool,
    no_docs: bool,
    cargo_args: &[String],
) -> Result<Idl> {
    let mut cmd = Command::new("cargo");
    if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
        install_toolchain_if_needed(&toolchain)?;
        cmd.arg("+{toolchain}");
    }

    let output = cmd
        .args([
            "test",
            "__anchor_private_print_idl",
            "--features",
            "idl-build",
        ])
        .args(cargo_args)
        .args(["--", "--show-output", "--quiet"])
        .env(
            "ANCHOR_IDL_BUILD_NO_DOCS",
            if no_docs { "TRUE" } else { "FALSE" },
        )
        .env(
            "ANCHOR_IDL_BUILD_RESOLUTION",
            if resolution { "TRUE" } else { "FALSE" },
        )
        .env(
            "ANCHOR_IDL_BUILD_SKIP_LINT",
            if skip_lint { "TRUE" } else { "FALSE" },
        )
        .env("ANCHOR_IDL_BUILD_PROGRAM_PATH", program_path)
        .env("RUSTFLAGS", "-A warnings")
        .current_dir(program_path)
        .stderr(Stdio::inherit())
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if env::var("ANCHOR_LOG").is_ok() {
        eprintln!("{stdout}");
    }

    if !output.status.success() {
        return Err(anyhow!(
            "Building IDL failed. Run `ANCHOR_LOG=true anchor idl build` to see the logs."
        ));
    }

    enum State {
        Pass,
        Address,
        Constants(Vec<String>),
        Events(Vec<String>),
        Errors(Vec<String>),
        Program(Vec<String>),
    }

    let mut address = String::new();
    let mut constants = vec![];
    let mut events = vec![];
    let mut errors = None;
    let mut types = BTreeMap::new();
    let mut idl: Option<Idl> = None;

    let mut state = State::Pass;
    for line in stdout.lines() {
        match &mut state {
            State::Pass => match line {
                "--- IDL begin address ---" => state = State::Address,
                "--- IDL begin const ---" => state = State::Constants(vec![]),
                "--- IDL begin event ---" => state = State::Events(vec![]),
                "--- IDL begin errors ---" if errors.is_some() => {
                    return Err(anyhow!("Multiple error definitions are not allowed."));
                }
                "--- IDL begin errors ---" => state = State::Errors(vec![]),
                "--- IDL begin program ---" => state = State::Program(vec![]),
                _ => {
                    if line.starts_with("test result: ok")
                        && !line.starts_with("test result: ok. 0 passed; 0 failed; 0")
                    {
                        if let Some(idl) = idl.as_mut() {
                            idl.address = mem::take(&mut address);
                            idl.constants = mem::take(&mut constants);
                            idl.events = mem::take(&mut events);
                            idl.errors = mem::take(&mut errors).unwrap_or_default();
                            idl.types = {
                                let prog_ty = mem::take(&mut idl.types);
                                let mut types = mem::take(&mut types);
                                types.extend(prog_ty.into_iter().map(|ty| (ty.name.clone(), ty)));
                                types.into_values().collect()
                            };
                        }
                    }
                }
            },
            State::Address => {
                address = line.replace(|c: char| !c.is_alphanumeric(), "");
                state = State::Pass;
                continue;
            }
            State::Constants(lines) => {
                if line == "--- IDL end const ---" {
                    let constant = serde_json::from_str(&lines.join("\n"))?;
                    constants.push(constant);
                    state = State::Pass;
                    continue;
                }

                lines.push(line.to_owned());
            }
            State::Events(lines) => {
                if line == "--- IDL end event ---" {
                    #[derive(Deserialize)]
                    struct IdlBuildEventPrint {
                        event: IdlEvent,
                        types: Vec<IdlTypeDef>,
                    }

                    let event = serde_json::from_str::<IdlBuildEventPrint>(&lines.join("\n"))?;
                    events.push(event.event);
                    types.extend(event.types.into_iter().map(|ty| (ty.name.clone(), ty)));
                    state = State::Pass;
                    continue;
                }

                lines.push(line.to_owned());
            }
            State::Errors(lines) => {
                if line == "--- IDL end errors ---" {
                    errors = Some(serde_json::from_str(&lines.join("\n"))?);
                    state = State::Pass;
                    continue;
                }

                lines.push(line.to_owned());
            }
            State::Program(lines) => {
                if line == "--- IDL end program ---" {
                    idl = Some(serde_json::from_str(&lines.join("\n"))?);
                    state = State::Pass;
                    continue;
                }

                lines.push(line.to_owned());
            }
        }
    }

    idl.ok_or_else(|| anyhow!("IDL doesn't exist"))
}

/// Install the given toolchain if it's not already installed.
fn install_toolchain_if_needed(toolchain: &str) -> Result<()> {
    let is_installed = Command::new("cargo")
        .arg(toolchain)
        .output()?
        .status
        .success();
    if !is_installed {
        Command::new("rustup")
            .args(["toolchain", "install", toolchain.trim_start_matches('+')])
            .spawn()?
            .wait()?;
    }

    Ok(())
}

/// Convert paths to name if there are no conflicts.
fn convert_module_paths(idl: Idl) -> Idl {
    let idl = serde_json::to_string(&idl).unwrap();
    let idl = Regex::new(r#""(\w+::)+(\w+)""#)
        .unwrap()
        .captures_iter(&idl.clone())
        .fold(idl, |acc, cur| {
            let path = cur.get(0).unwrap().as_str();
            let name = cur.get(2).unwrap().as_str();

            // Replace path with name
            let replaced_idl = acc.replace(path, &format!(r#""{name}""#));

            // Check whether there is a conflict
            let has_conflict = Regex::new(&format!(r#""(\w+::)+{name}""#))
                .unwrap()
                .is_match(&replaced_idl);
            if has_conflict {
                acc
            } else {
                replaced_idl
            }
        });

    serde_json::from_str(&idl).expect("Invalid IDL")
}

/// Alphabetically sort fields for consistency.
fn sort(mut idl: Idl) -> Idl {
    idl.accounts.sort_by(|a, b| a.name.cmp(&b.name));
    idl.constants.sort_by(|a, b| a.name.cmp(&b.name));
    idl.events.sort_by(|a, b| a.name.cmp(&b.name));
    idl.instructions.sort_by(|a, b| a.name.cmp(&b.name));
    idl.types.sort_by(|a, b| a.name.cmp(&b.name));

    idl
}

/// Verify IDL is valid.
fn verify(idl: &Idl) -> Result<()> {
    // Check empty discriminators
    macro_rules! check_empty_discriminators {
        ($field:ident) => {
            if let Some(item) = idl.$field.iter().find(|it| it.discriminator.is_empty()) {
                return Err(anyhow!(
                    "Empty discriminators are not allowed for {}: `{}`",
                    stringify!($field),
                    item.name
                ));
            }
        };
    }
    check_empty_discriminators!(accounts);
    check_empty_discriminators!(events);
    check_empty_discriminators!(instructions);

    // Check potential discriminator collisions
    macro_rules! check_discriminator_collision {
        ($field:ident) => {
            if let Some((outer, inner)) = idl.$field.iter().find_map(|outer| {
                idl.$field
                    .iter()
                    .filter(|inner| inner.name != outer.name)
                    .find(|inner| outer.discriminator.starts_with(&inner.discriminator))
                    .map(|inner| (outer, inner))
            }) {
                return Err(anyhow!(
                    "Ambiguous discriminators for {} `{}` and `{}`",
                    stringify!($field),
                    outer.name,
                    inner.name
                ));
            }
        };
    }
    check_discriminator_collision!(accounts);
    check_discriminator_collision!(events);
    check_discriminator_collision!(instructions);

    // Disallow all zero account discriminators
    if let Some(account) = idl
        .accounts
        .iter()
        .find(|acc| acc.discriminator.iter().all(|b| *b == 0))
    {
        return Err(anyhow!(
            "All zero account discriminators are not allowed (account: `{}`)",
            account.name
        ));
    }

    // Disallow account discriminators that can conflict with the `zero` constraint.
    //
    // Problematic scenario:
    //
    // 1. Account 1's discriminator starts with 0 (but not all 0s, since that's disallowed)
    // 2. Account 2's discriminator is a 1-byte custom discriminator
    // 3. Account 2 gets initialized using the `zero` constraint.
    //
    // In this case, it's possible to pass an already initialized Account 1 to a place that expects
    // non-initialized Account 2, because the first byte of Account 1 is also 0, which is what the
    // `zero` constraint checks.
    for account in &idl.accounts {
        let zero_count = account
            .discriminator
            .iter()
            .take_while(|b| **b == 0)
            .count();
        if let Some(account2) = idl
            .accounts
            .iter()
            .find(|acc| acc.discriminator.len() <= zero_count)
        {
            return Err(anyhow!(
                "Accounts may allow substitution when used with the `zero` constraint: `{}` `{}`",
                account.name,
                account2.name
            ));
        }
    }

    Ok(())
}
