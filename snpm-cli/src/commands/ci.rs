use anyhow::{Context, Result, bail};
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console};
use std::env;
use std::fs;
use std::path::PathBuf;

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

    // Resolve what the install will actually target before deleting
    // anything: wiping first would destroy node_modules even when ci
    // cannot run (no project, no lockfile), and would miss the real
    // project root when invoked from a subdirectory. npm ci likewise
    // validates the lockfile before it removes node_modules.
    let workspace = Workspace::discover(&cwd)?;
    let (wipe_roots, lockfile_root) = match &workspace {
        // `-w <name>` installs a single member, so only that member's
        // node_modules may be wiped — wiping siblings would leave them
        // deleted with nothing reinstalling them.
        Some(workspace) => match &args.workspace {
            Some(name) => {
                let member = workspace
                    .projects
                    .iter()
                    .find(|project| project.manifest.name.as_deref() == Some(name.as_str()))
                    .with_context(|| {
                        format!("workspace project {name} not found in {}", workspace.root.display())
                    })?;
                (vec![member.root.clone()], workspace.root.clone())
            }
            None => {
                let mut roots = vec![workspace.root.clone()];
                roots.extend(workspace.projects.iter().map(|project| project.root.clone()));
                (roots, workspace.root.clone())
            }
        },
        None => {
            let project = Project::discover(&cwd)?;
            (vec![project.root.clone()], project.root.clone())
        }
    };

    if !lockfile_root.join("snpm-lock.yaml").is_file()
        && snpm_core::lockfile::detect_compatible_lockfile(&lockfile_root).is_none()
    {
        bail!(
            "snpm ci requires an existing lockfile in {} (run `snpm install` first)",
            lockfile_root.display()
        );
    }

    // npm ci removes node_modules first for a from-scratch install. A
    // partial wipe must abort — installing over a half-deleted tree
    // silently breaks the clean-install contract this command exists for.
    for root in &wipe_roots {
        remove_node_modules(root)?;
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

fn remove_node_modules(root: &PathBuf) -> Result<()> {
    let node_modules = root.join("node_modules");
    if node_modules.is_dir() {
        fs::remove_dir_all(&node_modules)
            .with_context(|| format!("failed to remove {}", node_modules.display()))?;
    }
    Ok(())
}
