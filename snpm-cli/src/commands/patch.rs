use anyhow::Result;
use clap::{Args, Subcommand};
use snpm_core::{
    Project, SnpmConfig, console,
    operations::{commit_patch, list_project_patches, remove_package_patch, start_patch},
};
use std::env;
use std::path::{Path, PathBuf};

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
        PatchCommand::Edit { package } => run_edit(&package).await,
        PatchCommand::Commit { path } => run_commit(&path).await,
        PatchCommand::Remove { package } => run_remove(&package).await,
        PatchCommand::List => run_list().await,
    }
}

async fn run_edit(package: &str) -> Result<()> {
    console::header("patch edit", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;
    let project = Project::discover(&cwd)?;

    console::step(&format!("Preparing {} for patching...", package));

    let result = start_patch(&project, package)?;

    println!();
    println!(
        "Ready to patch {} @ {}",
        result.package_name, result.package_version
    );
    println!();
    println!("Edit the package at:");
    println!("  {}", result.edit_dir.display());
    println!();
    println!("When done, run:");
    println!("  snpm patch commit {}", result.edit_dir.display());
    println!();

    Ok(())
}

async fn run_commit(path: &Path) -> Result<()> {
    console::header("patch commit", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;
    let project = Project::discover(&cwd)?;

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    console::step("Creating patch from modifications...");

    let result = commit_patch(&project, path)?;

    println!();
    console::info(&format!(
        "Created patch for {} @ {}",
        result.package_name, result.package_version
    ));
    println!();
    println!("Patch saved to:");
    println!("  {}", result.patch_path.display());
    println!();
    println!("The patch will be applied automatically on `snpm install`");
    println!();

    Ok(())
}

async fn run_remove(package: &str) -> Result<()> {
    console::header("patch remove", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;
    let project = Project::discover(&cwd)?;

    console::step(&format!("Removing patch for {}...", package));

    match remove_package_patch(&project, package)? {
        Some(path) => {
            println!();
            console::info(&format!("Removed patch for {}", package));
            println!("  Deleted: {}", path.display());
            println!();
            println!("Run `snpm install` to restore the original package");
            println!();
        }
        None => {
            println!();
            console::warn(&format!("No patch found for {}", package));
            println!();
        }
    }

    Ok(())
}

async fn run_list() -> Result<()> {
    console::header("patch list", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;
    let project = Project::discover(&cwd)?;

    let patches = list_project_patches(&project)?;

    if patches.is_empty() {
        println!("No patches in this project");
        println!();
        println!("Create a patch with:");
        println!("  snpm patch edit <package>");
        return Ok(());
    }

    println!("Patches ({}):", patches.len());
    println!();

    for patch in patches {
        println!("  {} @ {}", patch.package_name, patch.package_version);
        println!("    {}", patch.patch_path.display());
    }

    println!();

    Ok(())
}
