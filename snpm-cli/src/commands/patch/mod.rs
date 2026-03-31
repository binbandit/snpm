mod commit;
mod edit;
mod list;
mod project;
mod remove;

use anyhow::Result;
use clap::{Args, Subcommand};
use snpm_core::{SnpmConfig, console};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(arg_required_else_help = true)]
pub struct PatchArgs {
    #[command(subcommand)]
    pub command: PatchCommand,
}

#[derive(Subcommand, Debug)]
pub enum PatchCommand {
    /// Prepare a package for patching
    #[command(alias = "start")]
    Edit {
        /// Package name (optionally with @version)
        package: String,
    },

    /// Create a patch from your modifications
    Commit {
        /// Path to the modified package directory
        path: PathBuf,
    },

    /// Remove a patch for a package
    Remove {
        /// Package name to remove patch for
        package: String,
    },

    /// List all patches in this project
    List,
}

pub async fn run(args: PatchArgs, _config: &SnpmConfig) -> Result<()> {
    match args.command {
        PatchCommand::Edit { package } => edit::run_edit(&package).await,
        PatchCommand::Commit { path } => commit::run_commit(&path).await,
        PatchCommand::Remove { package } => remove::run_remove(&package).await,
        PatchCommand::List => list::run_list().await,
    }
}

fn print_no_patch_hint() {
    println!("Create a patch with:");
    println!("  snpm patch edit <package>");
}

fn print_header(title: &str) {
    console::header(title, env!("CARGO_PKG_VERSION"));
}
