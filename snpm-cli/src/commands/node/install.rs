use anyhow::Result;
use clap::Args;
use snpm_core::node::{aliases, current, install, resolve};
use snpm_core::{SnpmConfig, console};

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Version selector: `20`, `20.10.0`, `v20`, `^20`, `lts`, `lts/iron`, or `latest`
    pub version: Option<String>,
    /// Install the latest LTS release
    #[arg(long = "lts")]
    pub lts: bool,
    /// Activate the installed version as the current default
    #[arg(long = "default")]
    pub set_default: bool,
}

pub async fn run(args: InstallArgs, config: &SnpmConfig) -> Result<()> {
    console::header("node install", env!("CARGO_PKG_VERSION"));

    let spec = pick_spec(&args)?;
    let resolved = resolve::resolve_spec(config, &spec, true).await?;
    let normalized = resolved.normalized.clone();

    let summary = install::install_version(config, &normalized).await?;

    if summary.already_installed {
        console::info(&format!("Node {} already installed", normalized));
    } else {
        console::info(&format!("Installed Node {}", normalized));
    }
    console::info(&format!("Binary: {}", summary.bin_path.display()));

    if args.set_default {
        aliases::write_alias(config, aliases::default_alias_name(), &normalized)?;
        current::write_current(config, &normalized)?;
        console::info(&format!("Set Node {} as default", normalized));
    }

    Ok(())
}

fn pick_spec(args: &InstallArgs) -> Result<String> {
    if args.lts {
        if let Some(provided) = args.version.as_deref() {
            if resolve::is_lts_selector(provided) {
                return Ok(provided.to_string());
            }
            anyhow::bail!("cannot combine --lts with explicit version '{provided}'");
        }
        return Ok("lts/*".to_string());
    }

    match args.version.as_deref() {
        Some(v) if !v.is_empty() => Ok(v.to_string()),
        _ => Ok("latest".to_string()),
    }
}
