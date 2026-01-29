use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Remove globally installed package
    #[arg(short = 'g', long = "global")]
    pub global: bool,
    /// Packages to remove
    pub packages: Vec<String>,
}

pub async fn run(args: RemoveArgs, config: &SnpmConfig) -> Result<()> {
    if args.global {
        console::header("remove --global", env!("CARGO_PKG_VERSION"));
        operations::remove_global(config, args.packages).await?;
        return Ok(());
    }

    console::header("remove", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;
    let mut project = Project::discover(&cwd)?;
    operations::remove(config, &mut project, args.packages).await?;

    Ok(())
}
