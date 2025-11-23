use anyhow::Result;
use clap::Parser;
use snpm_core::{
    Project, SnpmConfig, Workspace,
    operations::{self, InstallOptions},
};
use std::env;
use tracing_subscriber::EnvFilter;

mod cli;

use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;

    let args = Cli::parse();
    let config = SnpmConfig::from_env();

    match args.command {
        Command::Install {
            packages,
            production,
            frozen_lockfile,
            force,
        } => {
            let cwd = env::current_dir()?;
            if packages.is_empty() {
                if let Some(workspace) = Workspace::discover(&cwd)? {
                    if workspace.root == cwd {
                        for project in workspace.projects.iter() {
                            let options = operations::InstallOptions {
                                requested: Vec::new(),
                                dev: false,
                                include_dev: !production,
                                frozen_lockfile,
                                force,
                            };
                            operations::install(&config, project, options).await?;
                        }

                        return Ok(());
                    }
                }
            }

            let project = Project::discover(&cwd)?;
            let options = operations::InstallOptions {
                requested: packages,
                dev: false,
                include_dev: !production,
                frozen_lockfile,
                force,
            };
            operations::install(&config, &project, options).await?;
        }
        Command::Add {
            dev,
            workspace: target,
            packages,
            force,
        } => {
            let cwd = env::current_dir()?;

            if let Some(workspace_name) = target {
                let workspace = Workspace::discover(&cwd)?
                    .ok_or_else(|| anyhow::anyhow!("snpm add -w used outside a workspace"))?;

                let project = workspace
                    .projects
                    .iter()
                    .find(|p| p.manifest.name.as_deref() == Some(workspace_name.as_str()))
                    .ok_or_else(|| {
                        anyhow::anyhow!(format!("workspace project {workspace_name} not found"))
                    })?;

                let options = operations::InstallOptions {
                    requested: packages,
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                };
                operations::install(&config, project, options).await?;
            } else {
                let project = Project::discover(&cwd)?;
                let options = InstallOptions {
                    requested: packages,
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                };
                operations::install(&config, &project, options).await?;
            }
        }
        Command::Remove { packages } => {
            let cwd = env::current_dir()?;
            let mut project = Project::discover(&cwd)?;
            operations::remove(&config, &mut project, packages).await?;
        }
        Command::Run { script, args } => {
            let cwd = env::current_dir()?;
            let project = Project::discover(&cwd)?;
            operations::run_script(&project, &script, &args)?;
        }
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
