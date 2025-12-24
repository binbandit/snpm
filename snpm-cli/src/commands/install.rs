use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct InstallArgs {
    #[arg(long)]
    pub production: bool,
    #[arg(long = "frozen-lockfile", alias = "immutable")]
    pub frozen_lockfile: bool,
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    pub packages: Vec<String>,
    /// Target a specific workspace project by its package name
    #[arg(short = 'w', long = "workspace")]
    pub workspace: Option<String>,
}

pub async fn run(args: InstallArgs, config: &SnpmConfig) -> Result<()> {
    console::header("install", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if let Some(workspace_name) = args.workspace {
        let mut workspace = Workspace::discover(&cwd)?
            .ok_or_else(|| anyhow::anyhow!("snpm install -w used outside a workspace"))?;

        let project = workspace
            .projects
            .iter_mut()
            .find(|p| p.manifest.name.as_deref() == Some(workspace_name.as_str()))
            .ok_or_else(|| {
                anyhow::anyhow!(format!("workspace project {workspace_name} not found"))
            })?;

        let options = operations::InstallOptions {
            requested: args.packages,
            dev: false,
            include_dev: !args.production,
            frozen_lockfile: args.frozen_lockfile,
            force: args.force,
            silent_summary: false,
        };
        operations::install(config, project, options).await?;
    } else {
        if args.packages.is_empty() {
            if let Some(mut workspace) = Workspace::discover(&cwd)? {
                if workspace.root == cwd {
                    operations::install_workspace(
                        config,
                        &mut workspace,
                        !args.production,
                        args.frozen_lockfile,
                        args.force,
                    )
                    .await?;

                    return Ok(());
                }
            }
        }

        let mut project = Project::discover(&cwd)?;
        let options = operations::InstallOptions {
            requested: args.packages,
            dev: false,
            include_dev: !args.production,
            frozen_lockfile: args.frozen_lockfile,
            force: args.force,
            silent_summary: false,
        };
        operations::install(config, &mut project, options).await?;
    }

    Ok(())
}
