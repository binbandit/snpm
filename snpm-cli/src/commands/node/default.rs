use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::{aliases, current, install, resolve};

#[derive(Args, Debug)]
pub struct DefaultArgs {
    /// Version selector to mark as the default
    pub version: String,
    /// Skip activating the version (only update the alias)
    #[arg(long = "no-activate")]
    pub no_activate: bool,
}

pub async fn run(args: DefaultArgs, config: &SnpmConfig) -> Result<()> {
    let resolved = resolve::resolve_spec(config, &args.version, true).await?;
    let normalized = resolved.normalized.clone();

    install::install_version(config, &normalized).await?;

    aliases::write_alias(config, aliases::default_alias_name(), &normalized)?;
    if !args.no_activate {
        current::write_current(config, &normalized)?;
    }

    println!("Default Node version set to {}", normalized);
    Ok(())
}
