use anyhow::{Result, anyhow};
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Script name, e.g. "test"
    pub script: String,
    /// Run the script in all workspace projects
    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,
    /// Filter workspace projects by name (glob patterns like "app-*" are supported)
    #[arg(long = "filter")]
    pub filter: Vec<String>,
    /// Skip the automatic install check before running scripts
    #[arg(long = "skip-install")]
    pub skip_install: bool,
    /// Extra arguments passed to the script (use `--` to separate)
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: RunArgs, config: &SnpmConfig) -> Result<()> {
    console::header(&format!("run {}", args.script), env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if args.recursive || !args.filter.is_empty() {
        let workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow!("snpm run -r/--filter used outside a workspace"))?;

        if !args.skip_install {
            if let Some(first_project) = workspace.projects.first() {
                if operations::is_stale(first_project) {
                    let mut project_clone = first_project.clone();
                    operations::lazy_install(config, &mut project_clone).await?;
                }
            }
        }

        operations::run_workspace_scripts(&workspace, &args.script, &args.filter, &args.args)?;
    } else {
        let mut project = Project::discover(&cwd)?;

        if !args.skip_install && operations::is_stale(&project) {
            operations::lazy_install(config, &mut project).await?;
        }

        operations::run_script(&project, &args.script, &args.args)?;
    }

    Ok(())
}
