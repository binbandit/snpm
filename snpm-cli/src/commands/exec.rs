use anyhow::{Result, anyhow};
use clap::Args;
use snpm_core::{Project, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Command to execute
    pub command: String,
    /// Run in all workspace projects
    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,
    /// Filter workspace projects by name
    #[arg(long = "filter")]
    pub filter: Vec<String>,
    /// Arguments passed to the command
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: ExecArgs) -> Result<()> {
    console::header(&format!("exec {}", args.command), env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if args.recursive || !args.filter.is_empty() {
        let workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow!("snpm exec -r/--filter used outside a workspace"))?;

        operations::exec_workspace_command(&workspace, &args.command, &args.filter, &args.args)?;
    } else {
        let project = Project::discover(&cwd)?;
        operations::exec_command(&project, &args.command, &args.args)?;
    }

    Ok(())
}
