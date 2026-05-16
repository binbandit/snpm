use anyhow::{Context, Result};
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::{exec as node_exec, install, resolve, uninstall};

#[derive(Args, Debug)]
pub struct WhichArgs {
    /// Version selector (use --active to resolve via project pin / current / default)
    pub version: Option<String>,
    /// Resolve the version that snpm would activate in the current directory
    #[arg(long = "active", conflicts_with = "version")]
    pub active: bool,
    /// Print only the path (or nothing if no version is active)
    #[arg(long = "quiet")]
    pub quiet: bool,
}

pub async fn run(args: WhichArgs, config: &SnpmConfig) -> Result<()> {
    if args.active || args.version.is_none() {
        return print_active(config, args.quiet);
    }

    let spec = args
        .version
        .as_deref()
        .context("version is required when --active is not set")?;

    let normalized = match resolve::normalize_version(spec) {
        Some(version) => version,
        None => resolve::resolve_spec(config, spec, true).await?.normalized,
    };

    if !uninstall::is_version_installed(config, &normalized) {
        if args.quiet {
            return Ok(());
        }
        anyhow::bail!(
            "Node {} is not installed. Install with: snpm node install {}",
            normalized,
            normalized
        );
    }

    let version_dir = config.node_version_dir(&normalized);
    let bin_path = install::node_binary_path(&version_dir);
    println!("{}", bin_path.display());
    Ok(())
}

fn print_active(config: &SnpmConfig, quiet: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let active = node_exec::active_for_project_offline(config, &cwd)?;

    match active {
        Some(active) => {
            println!("{}", active.bin_dir.display());
            Ok(())
        }
        None => {
            if quiet {
                Ok(())
            } else {
                println!("No active Node version for the current directory.");
                Ok(())
            }
        }
    }
}
