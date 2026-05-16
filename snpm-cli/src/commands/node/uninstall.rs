use anyhow::{Context, Result};
use clap::Args;
use snpm_core::node::{resolve, uninstall};
use snpm_core::{SnpmConfig, console};

#[derive(Args, Debug)]
pub struct UninstallArgs {
    /// Version selector (exact like `20.10.0`, partial like `20`, or alias name)
    pub version: String,
}

pub fn run(args: UninstallArgs, config: &SnpmConfig) -> Result<()> {
    console::header("node uninstall", env!("CARGO_PKG_VERSION"));

    let normalized = resolve::normalize_version(&args.version)
        .or_else(|| match_installed(config, &args.version))
        .with_context(|| {
            format!(
                "cannot resolve '{}' to an installed Node version",
                args.version
            )
        })?;

    let summary = uninstall::uninstall_version(config, &normalized)?;

    if summary.removed_dir.is_some() {
        console::info(&format!("Removed Node {}", summary.version));
    } else {
        console::info(&format!("Node {} was not installed", summary.version));
    }

    if summary.cleared_current {
        console::info("Cleared current version pointer");
    }
    for alias in summary.removed_aliases {
        console::info(&format!("Removed alias '{}'", alias));
    }

    Ok(())
}

fn match_installed(config: &SnpmConfig, spec: &str) -> Option<String> {
    let installed = uninstall::list_installed_versions(config).ok()?;
    for version in installed {
        let candidate = version.strip_prefix('v').unwrap_or(&version);
        if candidate.starts_with(spec.trim_start_matches('v')) {
            return Some(version);
        }
    }
    None
}
