//! Command-line interface: argument parsing and dispatch.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::commands::{self, Config};
use crate::error::Result;
use crate::report::Format;

/// Catch outdated artifacts via a declared dependency graph.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Path to the manifest (default: `outdatty.yaml` or `outdatty.yml`).
    #[arg(long, global = true)]
    manifest: Option<PathBuf>,

    /// Path to the lockfile (default: `outdatty.lock` next to the manifest).
    #[arg(long, global = true)]
    lock: Option<PathBuf>,

    /// Output format.
    #[arg(long, global = true, default_value = "plain")]
    format: Format,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Write a starter manifest.
    Init {
        /// Overwrite an existing manifest.
        #[arg(long)]
        force: bool,
    },
    /// Verify that dependents are confirmed for every changed source.
    Check {
        /// Limit to the named group(s). Repeatable.
        #[arg(long = "group", value_name = "ID")]
        groups: Vec<String>,
    },
    /// Confirm the current state by refreshing the lockfile.
    Update {
        /// Limit to the named group(s). Repeatable.
        #[arg(long = "group", value_name = "ID")]
        groups: Vec<String>,
    },
    /// Show the status of every group without failing.
    Status {
        /// Limit to the named group(s). Repeatable.
        #[arg(long = "group", value_name = "ID")]
        groups: Vec<String>,
    },
    /// Print the JSON schema for the manifest.
    Schema,
}

/// Parses arguments and runs the requested command.
///
/// # Errors
///
/// Returns an error if the command fails. The returned [`ExitCode`]
/// distinguishes drift (1) from success (0); operational errors surface as
/// `Err` and are mapped to exit code 2 by the binary.
pub fn run() -> Result<ExitCode> {
    Cli::parse().dispatch()
}

impl Cli {
    fn config(&self) -> Config {
        Config {
            manifest: self.manifest.clone(),
            lock: self.lock.clone(),
            format: self.format,
        }
    }

    fn dispatch(&self) -> Result<ExitCode> {
        let config = self.config();
        match &self.command {
            Command::Schema => {
                print_output(&commands::schema()?);
                Ok(ExitCode::SUCCESS)
            }
            Command::Init { force } => {
                print_output(&commands::init(&config, *force)?);
                Ok(ExitCode::SUCCESS)
            }
            Command::Check { groups } => {
                let outcome = commands::check(&config, groups)?;
                print_output(&outcome.output);
                Ok(exit_code(outcome.failed))
            }
            Command::Update { groups } => {
                print_output(&commands::update(&config, groups)?);
                Ok(ExitCode::SUCCESS)
            }
            Command::Status { groups } => {
                print_output(&commands::status(&config, groups)?);
                Ok(ExitCode::SUCCESS)
            }
        }
    }
}

fn exit_code(has_failure: bool) -> ExitCode {
    if has_failure {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn print_output(text: &str) {
    if !text.is_empty() {
        print!("{text}");
    }
}
