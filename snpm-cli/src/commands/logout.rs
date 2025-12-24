use anyhow::Result;
use clap::Args;
use snpm_core::{SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct LogoutArgs {
    /// Registry URL to remove credentials for. Defaults to the current default registry.
    #[arg(long)]
    pub registry: Option<String>,
    /// Remove credentials for a specific scope (e.g., @myorg)
    #[arg(long)]
    pub scope: Option<String>,
}

pub async fn run(args: LogoutArgs, config: &SnpmConfig) -> Result<()> {
    console::header("logout", env!("CARGO_PKG_VERSION"));

    operations::logout(config, args.registry.as_deref(), args.scope.as_deref())?;

    console::info("Credentials removed successfully");
    Ok(())
}
