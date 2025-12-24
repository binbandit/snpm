use anyhow::{Result, anyhow};
use clap::Args;
use snpm_core::{Project, Workspace, console, operations};
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
    /// Extra arguments passed to the script (use `--` to separate)
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: RunArgs) -> Result<()> {
    console::header(&format!("run {}", args.script), env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if args.recursive || !args.filter.is_empty() {
        let workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow!("snpm run -r/--filter used outside a workspace"))?;

        operations::run_workspace_scripts(&workspace, &args.script, &args.filter, &args.args)?;
    } else {
        let project = Project::discover(&cwd)?;
        operations::run_script(&project, &args.script, &args.args)?;
    }

    Ok(())
}
