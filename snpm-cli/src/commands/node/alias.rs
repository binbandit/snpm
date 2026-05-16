use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::{aliases, resolve};

#[derive(Args, Debug)]
pub struct AliasArgs {
    /// Alias name to set (omit to list all aliases)
    pub name: Option<String>,
    /// Target version or alias (must be provided when `name` is given)
    pub target: Option<String>,
}

pub async fn run(args: AliasArgs, config: &SnpmConfig) -> Result<()> {
    match (args.name, args.target) {
        (None, _) => list(config),
        (Some(name), None) => print_one(config, &name),
        (Some(name), Some(target)) => set(config, &name, &target).await,
    }
}

fn list(config: &SnpmConfig) -> Result<()> {
    let entries = aliases::list_aliases(config)?;
    if entries.is_empty() {
        println!("No Node aliases defined yet.");
        println!();
        println!("Create one with: snpm node alias <name> <version>");
        return Ok(());
    }

    println!("Node aliases:");
    for entry in entries {
        println!("  {} -> {}", entry.name, entry.target);
    }
    Ok(())
}

fn print_one(config: &SnpmConfig, name: &str) -> Result<()> {
    match aliases::read_alias(config, name)? {
        Some(target) => {
            println!("{} -> {}", name, target);
            Ok(())
        }
        None => anyhow::bail!("alias '{name}' not found"),
    }
}

async fn set(config: &SnpmConfig, name: &str, target: &str) -> Result<()> {
    let normalized = match resolve::normalize_version(target) {
        Some(version) => version,
        None => {
            resolve::resolve_spec(config, target, true)
                .await?
                .normalized
        }
    };
    aliases::write_alias(config, name, &normalized)?;
    println!("Set alias '{}' -> {}", name, normalized);
    Ok(())
}
