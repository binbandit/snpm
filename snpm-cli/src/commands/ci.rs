use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{SnpmConfig, Workspace, console};
use std::env;
use std::fs;

use super::install::InstallArgs;

/// Clean, reproducible install for CI: wipe existing `node_modules` and
/// install strictly from the lockfile (frozen). Mirrors `npm ci`.
#[derive(Args, Debug)]
pub struct CiArgs {
    /// Skip devDependencies
    #[arg(long)]
    pub production: bool,
    /// Target a specific workspace project by its package name
    #[arg(short = 'w', long = "workspace")]
    pub workspace: Option<String>,
}

pub async fn run(args: CiArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;

    // npm ci removes node_modules first for a from-scratch install. Wipe
    // the root and every workspace member's node_modules.
    remove_node_modules(&cwd);
    if let Ok(Some(workspace)) = Workspace::discover(&cwd) {
        remove_node_modules(&workspace.root);
        for project in &workspace.projects {
            remove_node_modules(&project.root);
        }
    }

    console::verbose("ci: wiped node_modules, running frozen install");

    let install_args = InstallArgs {
        production: args.production,
        frozen_lockfile: true,
        no_frozen_lockfile: false,
        prefer_frozen_lockfile: false,
        fix_lockfile: false,
        force: false,
        packages: Vec::new(),
        workspace: args.workspace,
    };

    super::install::run(install_args, config).await
}

fn remove_node_modules(root: &std::path::Path) {
    let node_modules = root.join("node_modules");
    if node_modules.is_dir() {
        let _ = fs::remove_dir_all(&node_modules);
    }
}
