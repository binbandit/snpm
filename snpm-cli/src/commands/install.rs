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

pub async fn run(arguments: InstallArgs, config: &SnpmConfig) -> Result<()> {
    console::header("install", env!("CARGO_PKG_VERSION"));

    let current_directory = env::current_dir()?;

    if let Some(workspace_name) = arguments.workspace {
        let mut workspace = Workspace::discover(&current_directory)?
            .ok_or_else(|| anyhow::anyhow!("snpm install -w used outside a workspace"))?;

        let project = workspace
            .projects
            .iter_mut()
            .find(|project| project.manifest.name.as_deref() == Some(workspace_name.as_str()))
            .ok_or_else(|| {
                anyhow::anyhow!(format!("workspace project {workspace_name} not found"))
            })?;

        let options = operations::InstallOptions {
            requested: arguments.packages,
            dev: false,
            include_dev: !arguments.production,
            frozen_lockfile: arguments.frozen_lockfile,
            force: arguments.force,
            silent_summary: false,
        };
        operations::install(config, project, options).await?;
    } else {
        if arguments.packages.is_empty() {
            if let Some(mut workspace) = Workspace::discover(&current_directory)? {
                if workspace.root == current_directory {
                    operations::install_workspace(
                        config,
                        &mut workspace,
                        !arguments.production,
                        arguments.frozen_lockfile,
                        arguments.force,
                    )
                    .await?;

                    return Ok(());
                }
            }
        }

        let mut project = Project::discover(&current_directory)?;
        let options = operations::InstallOptions {
            requested: arguments.packages,
            dev: false,
            include_dev: !arguments.production,
            frozen_lockfile: arguments.frozen_lockfile,
            force: arguments.force,
            silent_summary: false,
        };
        operations::install(config, &mut project, options).await?;
    }

    Ok(())
}
