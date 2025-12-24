use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct AddArgs {
    #[arg(short = 'D', long = "dev")]
    pub dev: bool,
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    pub packages: Vec<String>,
    #[arg(short = 'w', long = "workspace")]
    pub workspace: Option<String>,
}

pub async fn run(args: AddArgs, config: &SnpmConfig) -> Result<()> {
    console::header("add", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if let Some(workspace_name) = args.workspace {
        let mut workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow::anyhow!("snpm add -w used outside a workspace"))?;

        let project = workspace
            .projects
            .iter_mut()
            .find(|p| p.manifest.name.as_deref() == Some(workspace_name.as_str()))
            .ok_or_else(|| {
                anyhow::anyhow!(format!("workspace project {workspace_name} not found"))
            })?;

        let options = operations::InstallOptions {
            requested: args.packages,
            dev: args.dev,
            include_dev: true,
            frozen_lockfile: false,
            force: args.force,
            silent_summary: false,
        };
        operations::install(config, project, options).await?;
    } else {
        let mut project = Project::discover(&cwd)?;
        let options = operations::InstallOptions {
            requested: args.packages,
            dev: args.dev,
            include_dev: true,
            frozen_lockfile: false,
            force: args.force,
            silent_summary: false,
        };
        operations::install(config, &mut project, options).await?;
    }

    Ok(())
}
