use super::workspace::{self as workspace_selector, WorkspaceSelection};
use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, console};
use std::env;
use std::fs;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// List globally installed packages
    #[arg(short = 'g', long = "global")]
    pub global: bool,
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

pub async fn run(args: ListArgs, config: &SnpmConfig) -> Result<()> {
    if args.global {
        console::header("list --global", env!("CARGO_PKG_VERSION"));
        list_global(config)?;
    } else {
        console::header("list", env!("CARGO_PKG_VERSION"));
        let cwd = env::current_dir().context("failed to determine current directory")?;

        if let Some(WorkspaceSelection {
            projects,
            filter_label,
        }) = workspace_selector::select_workspace_projects(
            &cwd,
            "list",
            args.recursive,
            &args.filter,
            &args.filter_prod,
        )? {
            console::info(&format!(
                "listing workspace packages matching {filter_label}"
            ));
            for (idx, project) in projects.into_iter().enumerate() {
                if idx > 0 {
                    println!();
                }
                let name = workspace_selector::project_label(&project);
                println!("{}:", name);
                list_local(&project)?;
            }
        } else {
            let project = Project::discover(&cwd)?;
            list_local(&project)?;
        }
    }

    Ok(())
}

fn list_global(config: &SnpmConfig) -> Result<()> {
    let global_dir = config.global_dir();
    let global_bin_dir = config.global_bin_dir();

    // The global space is a managed snpm project: its package.json
    // dependencies are the globally installed packages.
    let manifest_path = global_dir.join("package.json");
    let dependencies: Vec<(String, String)> = if manifest_path.is_file() {
        let project = Project::from_manifest_path(manifest_path)?;
        project
            .manifest
            .dependencies
            .iter()
            .map(|(name, range)| (name.clone(), range.clone()))
            .collect()
    } else {
        Vec::new()
    };

    // Packages installed by the pre-project layout live as bare dirs in
    // the global root until the next add/remove -g migrates them.
    let legacy = snpm_core::operations::global::legacy_global_packages(config);

    if dependencies.is_empty() && legacy.is_empty() {
        println!("No global packages installed");
        println!();
        println!("Install with: snpm add -g <package>");
        return Ok(());
    }

    println!("Global packages ({}):", global_dir.display());
    println!();

    for (name, range) in dependencies {
        let installed = read_package_version(&global_dir.join("node_modules").join(&name));
        match installed {
            Some(version) => println!("  {} @ {}", name, version),
            None => println!("  {} @ {} (not linked)", name, range),
        }
    }

    for (name, version) in legacy {
        match version {
            Some(version) => println!("  {} @ {} (legacy layout)", name, version),
            None => println!("  {} (legacy layout)", name),
        }
    }

    println!();
    println!("Binaries: {}", global_bin_dir.display());
    println!();
    console::info("Ensure the bin directory is in your PATH");

    Ok(())
}

fn list_local(project: &Project) -> Result<()> {
    let deps = &project.manifest.dependencies;
    let dev_deps = &project.manifest.dev_dependencies;

    if deps.is_empty() && dev_deps.is_empty() {
        println!("No dependencies");
        return Ok(());
    }

    if !deps.is_empty() {
        println!("dependencies:");
        for (name, range) in deps {
            println!("  {} @ {}", name, range);
        }
    }

    if !dev_deps.is_empty() {
        if !deps.is_empty() {
            println!();
        }
        println!("devDependencies:");
        for (name, range) in dev_deps {
            println!("  {} @ {}", name, range);
        }
    }

    Ok(())
}

fn read_package_version(package_dir: &std::path::Path) -> Option<String> {
    let package_json = package_dir.join("package.json");
    let content = fs::read_to_string(package_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("version")?.as_str().map(String::from)
}
