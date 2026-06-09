//! Utilities for interacting with the Solana [Program Metadata program](https://github.com/solana-program/program-metadata).
//! Used for storing program IDLs.

use std::{
    io,
    path::PathBuf,
    process::{Command, ExitStatus},
};

/// Corresponds to a version of the [program-metadata JS client](https://www.npmjs.com/package/@solana-program/program-metadata).
const PMP_CLIENT_VERSION: &str = "0.5.1";

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
        let mut command = Command::new("npx");
        // Force on first-time install
        command.arg("--yes");
        // Use pinned version
        command.arg(format!(
            "--package=@solana-program/program-metadata@{PMP_CLIENT_VERSION}"
        ));
        command.arg("--");
        command.arg("program-metadata");
        command.args(["--rpc", &self.rpc_url]);
        command.args(self.subcommand.args());
        command.status()
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
    fn args(&self) -> Vec<&str> {
        match self {
            IdlSubcommandKind::Funded {
                keypair_path,
                priority_fees_str,
                cmd,
            } => cmd.args(keypair_path, priority_fees_str.as_deref()),
            IdlSubcommandKind::Unfunded(cmd) => cmd.args(),
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

    WriteSecurityMetadata {
        program_id: String,
        security_path: PathBuf,
        payer: String,
    },
}

impl FundedIdlSubcommand {
    fn args<'a>(&'a self, keypair_path: &'a str, priority_fees: Option<&'a str>) -> Vec<&'a str> {
        let mut args = vec!["--keypair", keypair_path];
        if let Some(fees) = priority_fees {
            args.extend(["--priority-fees", fees]);
        }
        match self {
            FundedIdlSubcommand::Write {
                program_id,
                idl_filepath: filepath,
                non_canonical,
            } => {
                args.extend(["write", "idl", program_id, filepath]);
                if *non_canonical {
                    args.push("--non-canonical");
                }
            }
            FundedIdlSubcommand::Close { program_id, seed } => {
                args.extend(["close", seed, program_id]);
            }
            FundedIdlSubcommand::CreateBuffer { filepath } => {
                args.extend(["create-buffer", filepath]);
            }
            FundedIdlSubcommand::SetBufferAuthority {
                buffer,
                new_authority,
            } => {
                args.extend([
                    "set-buffer-authority",
                    buffer,
                    "--new-authority",
                    new_authority,
                ]);
            }
            FundedIdlSubcommand::WriteBuffer {
                program_id,
                buffer,
                seed,
                close_buffer,
            } => {
                args.extend(["write", seed, program_id, "--buffer", buffer]);
                if *close_buffer {
                    args.push("--close-buffer");
                }
            }
            FundedIdlSubcommand::WriteSecurityMetadata {
                program_id,
                security_path,
                payer,
            } => {
                args.extend([
                    "write",
                    "security",
                    program_id,
                    security_path.to_str().unwrap(),
                    "--payer",
                    payer,
                ]);
            }
        }
        args
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
    fn args(&self) -> Vec<&str> {
        let mut args = vec![];
        match self {
            UnfundedIdlSubcommand::Fetch {
                program_id,
                out,
                non_canonical,
            } => {
                args.extend(["fetch", "idl", program_id]);
                if let Some(o) = out.as_ref() {
                    args.extend(["-o", o]);
                }
                if *non_canonical {
                    args.push("--non-canonical");
                }
            }
        }
        args
    }
}
