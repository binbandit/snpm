use anyhow::{Context, Result, anyhow};
use clap::Args;
use snpm_core::operations::lazy::lazy_install_with_mode;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Command to execute
    pub command: String,
    /// Run command through shell (enables pipes, redirects, etc.)
    #[arg(short = 'c', long = "shell-mode")]
    pub shell_mode: bool,
    /// Run in all workspace projects
    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long = "filter")]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long = "filter-prod")]
    pub filter_prod: Vec<String>,
    /// Skip the automatic install check before executing
    #[arg(long = "skip-install")]
    pub skip_install: bool,
    /// Arguments passed to the command
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: ExecArgs, config: &SnpmConfig) -> Result<()> {
    console::header(&format!("exec {}", args.command), env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir().context("failed to determine current directory")?;
    let frozen_lockfile = super::frozen::resolve_frozen_lockfile_mode(config, None);

    let options = operations::ExecOptions {
        command: &args.command,
        args: &args.args,
        shell_mode: args.shell_mode,
    };

    if args.recursive || !args.filter.is_empty() || !args.filter_prod.is_empty() {
        let workspace = Workspace::discover(&cwd)?.ok_or_else(|| {
            anyhow!("snpm exec -r/--filter/--filter-prod used outside a workspace")
        })?;

        if !args.skip_install
            && let Some(first_project) = workspace.projects.first()
            && operations::is_stale(first_project)
        {
            let mut project_clone = first_project.clone();
            lazy_install_with_mode(
                config,
                &mut project_clone,
                frozen_lockfile.mode,
                frozen_lockfile.strict_no_lockfile,
            )
            .await?;
        }

        operations::exec_workspace_command(&workspace, &options, &args.filter, &args.filter_prod)?;
    } else {
        let mut project = Project::discover(&cwd)?;

        if !args.skip_install && operations::is_stale(&project) {
            lazy_install_with_mode(
                config,
                &mut project,
                frozen_lockfile.mode,
                frozen_lockfile.strict_no_lockfile,
            )
            .await?;
        }

        operations::exec_command(&project, &options)?;
    }

    Ok(())
}
