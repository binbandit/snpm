use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct OutdatedArgs {
    /// Skip devDependencies
    #[arg(long)]
    pub production: bool,
}

pub async fn run(args: OutdatedArgs, config: &SnpmConfig) -> Result<()> {
    console::header("outdated", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if let Some(workspace) = Workspace::discover(&cwd)? {
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

                let name = project_label(project);
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
        let name = project_label(&project);
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
