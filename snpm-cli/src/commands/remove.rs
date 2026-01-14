use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Packages to remove from package.json
    pub packages: Vec<String>,
}

pub async fn run(args: RemoveArgs, config: &SnpmConfig) -> Result<()> {
    console::header("remove", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;
    let mut project = Project::discover(&cwd)?;
    operations::remove(config, &mut project, args.packages).await?;

    Ok(())
}
