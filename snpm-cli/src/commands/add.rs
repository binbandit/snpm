use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;
use super::workspace::{self as workspace_selector, WorkspaceSelection};

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Save to devDependencies instead of dependencies
    #[arg(short = 'D', long = "dev")]
    pub dev: bool,
    /// Install globally
    #[arg(short = 'g', long = "global", conflicts_with = "workspace")]
    pub global: bool,
    /// Ignore cached state and force a full install
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    /// Packages to add
    pub packages: Vec<String>,
    /// Target a specific workspace project by its package name
    #[arg(short = 'w', long = "workspace")]
    pub workspace: Option<String>,
    /// Run in all workspace projects
    #[arg(short = 'r', long, conflicts_with = "workspace")]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long, conflicts_with = "workspace")]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long, conflicts_with = "workspace")]
    pub filter_prod: Vec<String>,
}

pub async fn run(args: AddArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let AddArgs {
        dev,
        global,
        force,
        workspace,
        recursive,
        filter,
        filter_prod,
        packages,
    } = args;

    if !global && workspace.is_none() {
        if let Some(WorkspaceSelection { projects, filter_label }) =
            workspace_selector::select_workspace_projects(&cwd, "add", recursive, &filter, &filter_prod)?
        {
            console::header("add", env!("CARGO_PKG_VERSION"));
            let requested = packages.clone();
            for (idx, mut project) in projects.into_iter().enumerate() {
                if idx > 0 {
                    println!();
                }
                let options = operations::InstallOptions {
                    requested: requested.clone(),
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                    silent_summary: false,
                };

                console::info(&format!(
                    "add {} in {} ({filter_label})",
                    options.requested.join(", "),
                    workspace_selector::project_label(&project)
                ));
                operations::install(config, &mut project, options).await?;
            }
            return Ok(());
        }
    }

    if global {
        console::header("add --global", env!("CARGO_PKG_VERSION"));
        operations::install_global(config, packages).await?;
        return Ok(());
    }

    console::header("add", env!("CARGO_PKG_VERSION"));

    if let Some(workspace_name) = workspace {
        let mut workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow::anyhow!("snpm add -w used outside a workspace"))?;

        let project = workspace
            .projects
            .iter_mut()
            .find(|p| p.manifest.name.as_deref() == Some(workspace_name.as_str()))
            .ok_or_else(|| anyhow::anyhow!("workspace project {workspace_name} not found"))?;

        let options = operations::InstallOptions {
            requested: packages,
            dev,
            include_dev: true,
            frozen_lockfile: false,
            force,
            silent_summary: false,
        };
        operations::install(config, project, options).await?;
    } else {
        let mut project = Project::discover(&cwd)?;
        let options = operations::InstallOptions {
            requested: packages,
            dev,
            include_dev: true,
            frozen_lockfile: false,
            force,
            silent_summary: false,
        };
        operations::install(config, &mut project, options).await?;
    }

    Ok(())
}
