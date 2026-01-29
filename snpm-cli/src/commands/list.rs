use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, console};
use std::env;
use std::fs;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// List globally installed packages
    #[arg(short = 'g', long = "global")]
    pub global: bool,
}

pub async fn run(args: ListArgs, config: &SnpmConfig) -> Result<()> {
    if args.global {
        console::header("list --global", env!("CARGO_PKG_VERSION"));
        list_global(config)?;
    } else {
        console::header("list", env!("CARGO_PKG_VERSION"));
        list_local()?;
    }

    Ok(())
}

fn list_global(config: &SnpmConfig) -> Result<()> {
    let global_dir = config.global_dir();
    let global_bin_dir = config.global_bin_dir();

    if !global_dir.exists() {
        println!("No global packages installed");
        println!();
        println!("Install with: snpm add -g <package>");
        return Ok(());
    }

    let entries: Vec<_> = fs::read_dir(&global_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();

    if entries.is_empty() {
        println!("No global packages installed");
        println!();
        println!("Install with: snpm add -g <package>");
        return Ok(());
    }

    println!("Global packages ({}):", global_dir.display());
    println!();

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        let version = read_package_version(&entry.path());
        match version {
            Some(v) => println!("  {} @ {}", name_str, v),
            None => println!("  {}", name_str),
        }
    }

    println!();
    println!("Binaries: {}", global_bin_dir.display());
    println!();
    console::info("Ensure the bin directory is in your PATH");

    Ok(())
}

fn list_local() -> Result<()> {
    let cwd = env::current_dir()?;
    let project = Project::discover(&cwd)?;

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
