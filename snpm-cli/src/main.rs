use anyhow::Result;
use clap::Parser;
use snpm_core::{
    Project, SnpmConfig, Workspace, console,
    operations::{self, InstallOptions},
};
use std::{env, fs, process};
use tracing_subscriber::EnvFilter;

mod cli;

use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        console::error(&format!("{error}"));
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    init_tracing()?;

    let args = Cli::parse();
    let config = SnpmConfig::from_env();

    match args.command {
        Command::Install {
            packages,
            production,
            frozen_lockfile,
            force,
            workspace: target,
        } => {
            let mut heading = String::from("install");
            if production {
                heading.push_str(" --production");
            }
            if frozen_lockfile {
                heading.push_str(" --frozen-lockfile");
            }
            if force {
                heading.push_str(" --force");
            }
            if let Some(ref name) = target {
                heading.push_str(" -w ");
                heading.push_str(name);
            }
            console::heading(&heading);

            let cwd = env::current_dir()?;

            if let Some(workspace_name) = target {
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
                    requested: packages,
                    dev: false,
                    include_dev: !production,
                    frozen_lockfile,
                    force,
                };
                operations::install(&config, project, options).await?;
            } else {
                if packages.is_empty() {
                    if let Some(mut workspace) = Workspace::discover(&cwd)? {
                        if workspace.root == cwd {
                            for (index, project) in workspace.projects.iter_mut().enumerate() {
                                if index > 0 {
                                    println!();
                                }

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
        }

        Command::Add {
            dev,
            workspace: target,
            packages,
            force,
        } => {
            let mut heading = String::from("add");
            if dev {
                heading.push_str(" -D");
            }
            if force {
                heading.push_str(" --force");
            }
            if let Some(name) = target.as_deref() {
                heading.push_str(" -w ");
                heading.push_str(name);
            }
            console::heading(&heading);

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
            let mut heading = String::from("remove");
            if !packages.is_empty() {
                heading.push(' ');
                for (index, name) in packages.iter().enumerate() {
                    if index > 0 {
                        heading.push(' ');
                    }
                    heading.push_str(name);
                }
            }
            console::heading(&heading);

            let cwd = env::current_dir()?;
            let mut project = Project::discover(&cwd)?;
            operations::remove(&config, &mut project, packages).await?;
        }

        Command::Run { script, args } => {
            console::heading(&format!("run {script}"));

            let cwd = env::current_dir()?;
            let project = Project::discover(&cwd)?;
            operations::run_script(&project, &script, &args)?;
        }

        Command::Init => {
            let cwd = env::current_dir()?;
            let name = cwd.file_name().and_then(|os| os.to_str()).unwrap_or(".");
            console::heading(&format!("init {name}"));

            operations::init(&cwd)?;
        }

        Command::Upgrade {
            production,
            force,
            packages,
        } => {
            let mut heading = String::from("upgrade");
            if production {
                heading.push_str(" --production");
            }
            if force {
                heading.push_str(" --force");
            }
            if !packages.is_empty() {
                heading.push(' ');
                heading.push_str(&packages.join(" "));
            }
            console::heading(&heading);

            let cwd = env::current_dir()?;

            // Targeted upgrade: operate on the single discovered project
            if !packages.is_empty() {
                let mut project = Project::discover(&cwd)?;
                operations::upgrade(&config, &mut project, packages, production, force).await?;
                return Ok(());
            }

            // Full upgrade (no packages): always operate on the *workspace* lockfile if present
            let mut project = Project::discover(&cwd)?;
            let workspace = Workspace::discover(&cwd)?;

            let lockfile_path = workspace
                .as_ref()
                .map(|w| w.root.join("snpm-lock.yaml"))
                .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

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

            // If we're at the workspace root, upgrade all projects in the workspace
            if let Some(mut workspace) = workspace {
                if workspace.root == cwd {
                    for (index, member) in workspace.projects.iter_mut().enumerate() {
                        if index > 0 {
                            println!();
                        }

                        operations::install(&config, member, options.clone()).await?;
                    }

                    return Ok(());
                }
            }

            // Otherwise, upgrade just this project, but still using the workspace‑wide lockfile
            operations::install(&config, &mut project, options).await?;
        }

        Command::Outdated { production } => {
            let mut heading = String::from("outdated");
            if production {
                heading.push_str(" --production");
            }
            console::heading(&heading);

            let cwd = env::current_dir()?;

            if let Some(workspace) = Workspace::discover(&cwd)? {
                if workspace.root == cwd {
                    let mut any = false;

                    for project in workspace.projects.iter() {
                        let entries =
                            operations::outdated(&config, project, !production, false).await?;

                        if entries.is_empty() {
                            continue;
                        }

                        if any {
                            println!();
                        }
                        any = true;

                        let name = project_label(project);
                        console::project(&name);
                        print_outdated(&entries);
                    }

                    if !any {
                        console::info("All dependencies are up to date.");
                    }

                    return Ok(());
                }
            }

            let project = Project::discover(&cwd)?;
            let entries = operations::outdated(&config, &project, !production, false).await?;

            if entries.is_empty() {
                console::info("All dependencies are up to date.");
            } else {
                let name = project_label(&project);
                console::project(&name);
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

fn project_label(project: &Project) -> String {
    if let Some(name) = project.manifest.name.as_deref() {
        name.to_string()
    } else {
        project
            .root
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(".")
            .to_string()
    }
}
