use super::workspace::{self as workspace_selector, WorkspaceSelection};
use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct OutdatedArgs {
    /// Skip devDependencies
    #[arg(long)]
    pub production: bool,
    /// Run in all workspace projects
    #[arg(short = 'r', long)]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long)]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long)]
    pub filter_prod: Vec<String>,
}

pub async fn run(args: OutdatedArgs, config: &SnpmConfig) -> Result<()> {
    console::header("outdated", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir().context("failed to determine current directory")?;

    if let Some(WorkspaceSelection {
        projects,
        filter_label,
    }) = workspace_selector::select_workspace_projects(
        &cwd,
        "outdated",
        args.recursive,
        &args.filter,
        &args.filter_prod,
    )? {
        let mut any = false;

        for project in projects.into_iter() {
            let entries = operations::outdated(config, &project, !args.production, false).await?;
            if entries.is_empty() {
                continue;
            }

            if any {
                println!();
            }
            any = true;

            let name = workspace_selector::project_label(&project);
            println!("{}", name);
            println!("({})", filter_label);
            print_outdated(&entries);
        }

        if !any {
            console::info("All dependencies are up to date.");
        }

        return Ok(());
    }

    if let Some(workspace) = snpm_core::Workspace::discover(&cwd)? {
        if workspace.root == cwd {
            let mut any = false;

            for project in workspace.projects.iter() {
                let entries =
                    operations::outdated(config, project, !args.production, false).await?;

                if entries.is_empty() {
                    continue;
                }

                if any {
                    println!();
                }
                any = true;

                let name = workspace_selector::project_label(project);
                println!("\n{}", name);
                print_outdated(&entries);
            }

            if !any {
                console::info("All dependencies are up to date.");
            }

            return Ok(());
        }
    }

    let project = Project::discover(&cwd)?;
    let entries = operations::outdated(config, &project, !args.production, false).await?;

    if entries.is_empty() {
        console::info("All dependencies are up to date.");
    } else {
        let name = workspace_selector::project_label(&project);
        println!("\n{}", name);
        print_outdated(&entries);
    }

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
