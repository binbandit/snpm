use super::workspace::{self as workspace_selector, WorkspaceSelection};
use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Remove globally installed package
    #[arg(short = 'g', long = "global")]
    pub global: bool,
    /// Run in all workspace projects
    #[arg(short = 'r', long)]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long)]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long)]
    pub filter_prod: Vec<String>,
    /// Packages to remove
    pub packages: Vec<String>,
}

pub async fn run(args: RemoveArgs, config: &SnpmConfig) -> Result<()> {
    if args.global {
        console::header("remove --global", env!("CARGO_PKG_VERSION"));
        operations::remove_global(config, args.packages).await?;
        return Ok(());
    }

    console::header("remove", env!("CARGO_PKG_VERSION"));
    let frozen_lockfile = super::frozen::resolve_frozen_lockfile_mode(config, None);

    let cwd = env::current_dir().context("failed to determine current directory")?;

    if let Some(WorkspaceSelection {
        projects,
        filter_label,
    }) = workspace_selector::select_workspace_projects(
        &cwd,
        "remove",
        args.recursive,
        &args.filter,
        &args.filter_prod,
    )? {
        for (idx, mut project) in projects.into_iter().enumerate() {
            if idx > 0 {
                println!();
            }
            console::info(&format!(
                "remove {} in {} ({})",
                args.packages.join(", "),
                workspace_selector::project_label(&project),
                filter_label
            ));
            operations::remove(
                config,
                &mut project,
                args.packages.clone(),
                frozen_lockfile.mode,
                frozen_lockfile.strict_no_lockfile,
            )
            .await?;
        }
        return Ok(());
    }

    let mut project = Project::discover(&cwd)?;
    operations::remove(
        config,
        &mut project,
        args.packages,
        frozen_lockfile.mode,
        frozen_lockfile.strict_no_lockfile,
    )
    .await?;

    Ok(())
}
