use super::workspace::{self as workspace_selector, WorkspaceSelection};
use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;
use std::fs;

#[derive(Args, Debug)]
pub struct UpgradeArgs {
    /// Skip devDependencies
    #[arg(long)]
    pub production: bool,
    /// Ignore cached state and force a full resolve
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    /// Run in all workspace projects
    #[arg(short = 'r', long)]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long)]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long)]
    pub filter_prod: Vec<String>,
    /// Packages to upgrade (omit to refresh the lockfile and reinstall)
    pub packages: Vec<String>,
}

pub async fn run(args: UpgradeArgs, config: &SnpmConfig) -> Result<()> {
    console::header("upgrade", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir().context("failed to determine current directory")?;
    let frozen_lockfile = super::frozen::resolve_frozen_lockfile_mode(config, None);

    if let Some(WorkspaceSelection {
        projects,
        filter_label,
    }) = workspace_selector::select_workspace_projects(
        &cwd,
        "upgrade",
        args.recursive,
        &args.filter,
        &args.filter_prod,
    )? {
        let targets = if args.packages.is_empty() {
            "all".to_string()
        } else {
            args.packages.join(", ")
        };
        for (idx, mut project) in projects.into_iter().enumerate() {
            if idx > 0 {
                println!();
            }
            console::info(&format!(
                "upgrade {} in {} ({})",
                targets,
                workspace_selector::project_label(&project),
                filter_label
            ));
            operations::upgrade(
                config,
                &mut project,
                args.packages.clone(),
                frozen_lockfile.mode,
                frozen_lockfile.strict_no_lockfile,
                args.production,
                args.force,
            )
            .await?;
        }
        return Ok(());
    }

    if !args.packages.is_empty() {
        let mut project = Project::discover(&cwd)?;
        operations::upgrade(
            config,
            &mut project,
            args.packages,
            frozen_lockfile.mode,
            frozen_lockfile.strict_no_lockfile,
            args.production,
            args.force,
        )
        .await?;
        return Ok(());
    }

    let workspace = Workspace::discover(&cwd)?;

    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| cwd.join("snpm-lock.yaml"));

    if lockfile_path.is_file() {
        fs::remove_file(&lockfile_path)
            .with_context(|| format!("failed to remove lockfile {}", lockfile_path.display()))?;
    }

    if let Some(mut workspace) = workspace {
        operations::install_workspace(
            config,
            &mut workspace,
            !args.production,
            frozen_lockfile.mode,
            frozen_lockfile.strict_no_lockfile,
            args.force,
        )
        .await?;

        return Ok(());
    }

    let mut project = Project::discover(&cwd)?;

    let options = operations::InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev: !args.production,
        frozen_lockfile: frozen_lockfile.mode,
        strict_no_lockfile: frozen_lockfile.strict_no_lockfile,
        force: args.force,
        silent_summary: false,
    };

    operations::install(config, &mut project, options).await?;

    Ok(())
}
