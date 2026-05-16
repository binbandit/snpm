use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::{aliases, current, uninstall};

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Also show resolved alias targets
    #[arg(long = "with-aliases")]
    pub with_aliases: bool,
}

pub fn run(args: ListArgs, config: &SnpmConfig) -> Result<()> {
    let versions = uninstall::list_installed_versions(config)?;
    let active = current::read_current(config)?;
    let default_alias = aliases::read_alias(config, aliases::default_alias_name())?;

    if versions.is_empty() {
        println!("No Node versions installed yet.");
        println!();
        println!("Install one with: snpm node install --lts");
        return Ok(());
    }

    println!("Installed Node versions:");
    for version in &versions {
        let marker_active = active.as_deref() == Some(version.as_str());
        let marker_default = default_alias
            .as_deref()
            .map(|target| same_version(target, version))
            .unwrap_or(false);

        let mut markers: Vec<&str> = Vec::new();
        if marker_active {
            markers.push("active");
        }
        if marker_default {
            markers.push("default");
        }

        if markers.is_empty() {
            println!("  {}", version);
        } else {
            println!("  {} ({})", version, markers.join(", "));
        }
    }

    if args.with_aliases {
        let entries = aliases::list_aliases(config)?;
        if !entries.is_empty() {
            println!();
            println!("Aliases:");
            for entry in entries {
                println!("  {} -> {}", entry.name, entry.target);
            }
        }
    }

    Ok(())
}

fn same_version(target: &str, version_with_v: &str) -> bool {
    let stripped_target = target.trim().trim_start_matches('v');
    let stripped_version = version_with_v.trim().trim_start_matches('v');
    stripped_target == stripped_version
}
