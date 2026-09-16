//! Utilities for interacting with the Solana [Program Metadata program](https://github.com/solana-program/program-metadata).
//! Used for storing program IDLs.

use std::{
    io,
    process::{Command, ExitStatus},
};

/// Corresponds to a version of the [program-metadata JS client](https://www.npmjs.com/package/@solana-program/program-metadata).
const PMP_CLIENT_VERSION: &str = "0.5.1";

pub enum MetadataCommand {
    Funded {
        keypair_path: String,
        /// Separate fee/storage payer (`--payer`); `None` uses the keypair as payer.
        payer: Option<String>,
        priority_fees: Option<String>,
        args: Vec<String>,
    },
    Unfunded {
        args: Vec<String>,
    },
}

impl MetadataCommand {
    fn status(self, rpc_url: &str) -> io::Result<ExitStatus> {
        let mut command = Command::new("npx");
        // Force on first-time install
        command.arg("--yes");
        // Use pinned version
        command.arg(format!(
            "--package=@solana-program/program-metadata@{PMP_CLIENT_VERSION}"
        ));
        command.arg("--");
        command.arg("program-metadata");
        command.args(["--rpc", rpc_url]);
        match self {
            MetadataCommand::Funded {
                keypair_path,
                payer,
                priority_fees,
                args,
            } => {
                command.args(["--keypair", &keypair_path]);

                if let Some(payer) = payer {
                    command.args(["--payer", &payer]);
                }

                if let Some(priority_fee) = priority_fees {
                    command.args(["--priority-fees", &priority_fee]);
                }

                command.args(args);
            }

            MetadataCommand::Unfunded { args } => {
                command.args(args);
            }
        };
        command.status()
    }
}

pub struct IdlCommand {
    rpc_url: String,
    subcommand: IdlSubcommandKind,
}

impl IdlCommand {
    pub fn funded(
        rpc_url: String,
        keypair_path: String,
        priority_fees: Option<u64>,
        cmd: FundedIdlSubcommand,
    ) -> Self {
        let priority_fees_str = priority_fees.map(|f| f.to_string());
        Self {
            rpc_url,
            subcommand: IdlSubcommandKind::Funded {
                keypair_path,
                priority_fees_str,
                cmd,
            },
        }
    }

    pub fn unfunded(rpc_url: String, cmd: UnfundedIdlSubcommand) -> Self {
        Self {
            rpc_url,
            subcommand: IdlSubcommandKind::Unfunded(cmd),
        }
    }

    pub fn status(self) -> io::Result<ExitStatus> {
        let Self {
            rpc_url,
            subcommand,
        } = self;
        subcommand.into_metadata().status(&rpc_url)
    }
}

pub enum SecurityCommand {
    Write {
        program_id: String,
        security_path: String,
        /// Program upgrade authority signs to authorize the write
        keypair_path: String,
        /// Fee payer for the write
        payer: String,
        priority_fees: Option<String>,
    },
}

impl SecurityCommand {
    pub fn status(self, rpc_url: &str) -> io::Result<ExitStatus> {
        self.into_metadata().status(rpc_url)
    }

    fn into_metadata(self) -> MetadataCommand {
        let args = self.args();
        match self {
            Self::Write {
                keypair_path,
                payer,
                priority_fees,
                ..
            } => MetadataCommand::Funded {
                keypair_path,
                payer: Some(payer),
                priority_fees,
                args,
            },
        }
    }

    /// The domain-specific tail only (`write security <id> <file>`). Funding flags
    /// (`--keypair` / `--payer` / `--priority-fees`) are added by `MetadataCommand::Funded`.
    fn args(&self) -> Vec<String> {
        let parts: Vec<&str> = match self {
            Self::Write {
                program_id,
                security_path,
                ..
            } => vec!["write", "security", program_id, security_path],
        };
        parts.into_iter().map(String::from).collect()
    }
}

pub enum IdlSubcommandKind {
    /// IDL commands requiring funding, i.e. those that perform writes
    Funded {
        keypair_path: String,
        priority_fees_str: Option<String>,
        cmd: FundedIdlSubcommand,
    },
    /// IDL commands requiring no funding, i.e. readonly commands
    Unfunded(UnfundedIdlSubcommand),
}

impl IdlSubcommandKind {
    fn into_metadata(self) -> MetadataCommand {
        match self {
            IdlSubcommandKind::Funded {
                keypair_path,
                priority_fees_str,
                cmd,
            } => MetadataCommand::Funded {
                keypair_path,
                payer: None,
                priority_fees: priority_fees_str,
                args: cmd.args(),
            },
            IdlSubcommandKind::Unfunded(cmd) => MetadataCommand::Unfunded { args: cmd.args() },
        }
    }
}

pub enum FundedIdlSubcommand {
    Write {
        program_id: String,
        idl_filepath: String,
        non_canonical: bool,
    },
    Close {
        program_id: String,
        seed: String,
    },
    CreateBuffer {
        filepath: String,
    },
    SetBufferAuthority {
        buffer: String,
        new_authority: String,
    },
    WriteBuffer {
        program_id: String,
        buffer: String,
        seed: String,
        close_buffer: bool,
    },
}

impl FundedIdlSubcommand {
    /// The domain-specific tail only (e.g. `write idl <id> <file>`). Funding flags
    /// (`--keypair` / `--priority-fees`) are added by `MetadataCommand::Funded`
    fn args(&self) -> Vec<String> {
        let parts: Vec<&str> = match self {
            FundedIdlSubcommand::Write {
                program_id,
                idl_filepath,
                non_canonical,
            } => {
                let mut parts = vec!["write", "idl", program_id, idl_filepath];
                if *non_canonical {
                    parts.push("--non-canonical");
                }
                parts
            }
            FundedIdlSubcommand::Close { program_id, seed } => vec!["close", seed, program_id],
            FundedIdlSubcommand::CreateBuffer { filepath } => vec!["create-buffer", filepath],
            FundedIdlSubcommand::SetBufferAuthority {
                buffer,
                new_authority,
            } => vec![
                "set-buffer-authority",
                buffer,
                "--new-authority",
                new_authority,
            ],
            FundedIdlSubcommand::WriteBuffer {
                program_id,
                buffer,
                seed,
                close_buffer,
            } => {
                let mut parts = vec!["write", seed, program_id, "--buffer", buffer];
                if *close_buffer {
                    parts.push("--close-buffer");
                }
                parts
            }
        };
        parts.into_iter().map(String::from).collect()
    }
}

pub enum UnfundedIdlSubcommand {
    Fetch {
        program_id: String,
        out: Option<String>,
        non_canonical: bool,
    },
}

impl UnfundedIdlSubcommand {
    fn args(&self) -> Vec<String> {
        let parts: Vec<&str> = match self {
            UnfundedIdlSubcommand::Fetch {
                program_id,
                out,
                non_canonical,
            } => {
                let mut parts = vec!["fetch", "idl", program_id];
                if let Some(o) = out.as_ref() {
                    parts.extend(["-o", o]);
                }
                if *non_canonical {
                    parts.push("--non-canonical");
                }
                parts
            }
        };
        parts.into_iter().map(String::from).collect()
    }
}
