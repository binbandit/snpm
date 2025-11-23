use anyhow::Result;
use clap::Parser;
use snpm_core::{
    Project, SnpmConfig, Workspace,
    operations::{self, InstallOptions},
};
use std::{env, fs};
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
                if let Some(mut workspace) = Workspace::discover(&cwd)? {
                    if workspace.root == cwd {
                        for project in workspace.projects.iter_mut() {
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

            let mut project = Project::discover(&cwd)?;
            let options = operations::InstallOptions {
                requested: packages,
                dev: false,
                include_dev: !production,
                frozen_lockfile,
                force,
            };
            operations::install(&config, &mut project, options).await?;
        }

        Command::Add {
            dev,
            workspace: target,
            packages,
            force,
        } => {
            let cwd = env::current_dir()?;

            if let Some(workspace_name) = target {
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
                    requested: packages,
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                };
                operations::install(&config, project, options).await?;
            } else {
                let mut project = Project::discover(&cwd)?;
                let options = InstallOptions {
                    requested: packages,
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                };
                operations::install(&config, &mut project, options).await?;
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
        Command::Init => {
            let cwd = env::current_dir()?;
            operations::init(&cwd)?;
        }
        Command::Upgrade { production, force } => {
            let cwd = env::current_dir()?;

            if let Some(mut workspace) = Workspace::discover(&cwd)? {
                if workspace.root == cwd {
                    let lockfile_path = workspace.root.join("snpm-lock.yaml");
                    if lockfile_path.is_file() {
                        fs::remove_file(&lockfile_path)?;
                    }

                    for project in workspace.projects.iter_mut() {
                        let options = InstallOptions {
                            requested: Vec::new(),
                            dev: false,
                            include_dev: !production,
                            frozen_lockfile: false,
                            force,
                        };
                        operations::install(&config, project, options).await?;
                    }

                    return Ok(());
                }
            }

            let mut project = Project::discover(&cwd)?;
            let lockfile_path = project.root.join("snpm-lockfile.yaml");
            if lockfile_path.is_file() {
                fs::remove_file(&lockfile_path)?;
            }

            let options = InstallOptions {
                requested: Vec::new(),
                dev: false,
                include_dev: !production,
                frozen_lockfile: false,
                force,
            };
            operations::install(&config, &mut project, options).await?;
        }
        Command::Outdated { production } => {
            let cwd = env::current_dir()?;

            if let Some(workspace) = Workspace::discover(&cwd)? {
                if workspace.root == cwd {
                    let mut any = false;

                    for project in workspace.projects.iter() {
                        let entries = operations::outdated(&config, project, !production).await?;

                        if entries.is_empty() {
                            continue;
                        }

                        any = true;

                        let name = project
                            .manifest
                            .name
                            .as_deref()
                            .map(std::borrow::Cow::Borrowed)
                            .unwrap_or_else(|| project.root.to_string_lossy());
                        println!("{name}:");
                        print_outdated(&entries);
                        println!();
                    }

                    if !any {
                        println!("All dependencies are up to date.");
                    }

                    return Ok(());
                }
            }

            let project = Project::discover(&cwd)?;
            let entries = operations::outdated(&config, &project, !production).await?;

            if entries.is_empty() {
                println!("All dependencies are up to date");
            } else {
                print_outdated(&entries);
            }
        }
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

fn print_outdated(entries: &[operations::OutdatedEntry]) {
    if entries.is_empty() {
        return;
    }

    let mut name_width = 4;

    for entry in entries {
        if entry.name.len() > name_width {
            name_width = entry.name.len();
        }
    }

    let header_name = "name";
    let header_current = "current";
    let header_wanted = "wanted";

    println!(
        "{:<name_width$}  {:<10}  {:<10}",
        header_name,
        header_current,
        header_wanted,
        name_width = name_width
    );

    for entry in entries {
        let current = entry.current.as_deref().unwrap_or("-");
        println!(
            "{:<name_width$}  {:<10}  {:<10}",
            entry.name,
            current,
            entry.wanted,
            name_width = name_width
        );
    }
}
