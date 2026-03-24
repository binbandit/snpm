use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, operations};
use std::env;

#[derive(Args, Debug)]
pub struct UnlinkArgs {
    /// Package name to unlink (omit to unlink current package globally)
    pub package: Option<String>,
}

pub async fn run(args: UnlinkArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    match args.package {
        Some(package_name) => {
            operations::unlink_local(&project, &package_name)?;
        }
        None => {
            operations::unlink_global(config, &project)?;
        }
    }

    Ok(())
}
