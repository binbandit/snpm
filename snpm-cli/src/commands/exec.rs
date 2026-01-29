use anyhow::{Result, anyhow};
use clap::Args;
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
    /// Filter workspace projects by name
    #[arg(long = "filter")]
    pub filter: Vec<String>,
    /// Skip the automatic install check before executing
    #[arg(long = "skip-install")]
    pub skip_install: bool,
    /// Arguments passed to the command
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: ExecArgs, config: &SnpmConfig) -> Result<()> {
    console::header(&format!("exec {}", args.command), env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    let options = operations::ExecOptions {
        command: &args.command,
        args: &args.args,
        shell_mode: args.shell_mode,
    };

    if args.recursive || !args.filter.is_empty() {
        let workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow!("snpm exec -r/--filter used outside a workspace"))?;

        if !args.skip_install {
            for project in &workspace.projects {
                if operations::is_stale(project) {
                    let mut project_clone = project.clone();
                    operations::lazy_install(config, &mut project_clone).await?;
                    break;
                }
            }
        }

        operations::exec_workspace_command(&workspace, &options, &args.filter)?;
    } else {
        let mut project = Project::discover(&cwd)?;

        if !args.skip_install && operations::is_stale(&project) {
            operations::lazy_install(config, &mut project).await?;
        }

        operations::exec_command(&project, &options)?;
    }

    Ok(())
}
