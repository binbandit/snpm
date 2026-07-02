use anyhow::Result;
use clap::Args;
use anyhow::bail;
use snpm_core::node::{aliases, current, install, resolve, uninstall};
use snpm_core::{SnpmConfig, console};

#[derive(Args, Debug)]
pub struct UseArgs {
    /// Version selector (defaults to reading `.node-version` / `.nvmrc` / `engines.node`)
    pub version: Option<String>,
    /// Use the newest LTS release
    #[arg(long = "lts")]
    pub lts: bool,
    /// Also persist this version as the default
    #[arg(long = "default")]
    pub set_default: bool,
    /// Install the version on demand if not already installed
    #[arg(long = "install")]
    pub install: bool,
    /// Suppress informational output (useful for shell hooks)
    #[arg(long = "silent")]
    pub silent: bool,
}

pub async fn run(args: UseArgs, config: &SnpmConfig) -> Result<()> {
    if !args.silent {
        console::header("node use", env!("CARGO_PKG_VERSION"));
    }

    let spec = pick_spec(&args, config)?;
    let resolved = resolve::resolve_spec(config, &spec, true).await?;
    let normalized = resolved.normalized.clone();

    if !args.install && !uninstall::is_version_installed(config, &normalized) {
        bail!(
            "Node {normalized} is not installed; run `snpm node install {normalized}` \
             or pass --install"
        );
    }

    let summary = install::install_version(config, &normalized).await?;
    if !summary.already_installed && !args.silent {
        console::info(&format!("Installed Node {} on demand", normalized));
    }

    current::write_current(config, &normalized)?;

    if args.set_default {
        aliases::write_alias(config, aliases::default_alias_name(), &normalized)?;
    }

    if !args.silent {
        console::info(&format!("Active Node version: {}", normalized));
        console::info(&format!("Binary: {}", summary.bin_path.display()));
    }

    Ok(())
}

fn pick_spec(args: &UseArgs, config: &SnpmConfig) -> Result<String> {
    if args.lts {
        return Ok("lts/*".to_string());
    }

    if let Some(version) = args.version.as_deref().filter(|v| !v.is_empty()) {
        return Ok(version.to_string());
    }

    let cwd = std::env::current_dir()?;
    if let Some(pin) = snpm_core::node::discover::discover_pinned(&cwd)? {
        return Ok(pin.spec);
    }

    if let Some(target) = aliases::read_alias(config, aliases::default_alias_name())? {
        return Ok(target);
    }

    anyhow::bail!(
        "no version supplied and no `.node-version`/`.nvmrc`/`engines.node` found in this tree"
    )
}
