use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, operations};
use std::env;

#[derive(Args, Debug)]
pub struct LinkArgs {
    /// Package name to link into the current project (omit to link current package globally)
    pub package: Option<String>,
}

pub async fn run(args: LinkArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    match args.package {
        Some(package_name) => {
            operations::link_local(&project, config, &package_name)?;
        }
        None => {
            operations::link_global(config, &project)?;
        }
    }

    Ok(())
}
