use anyhow::Result;
use clap::Parser;
use snpm_core::{Project, SnpmConfig, operations};
use std::env;
use tracing_subscriber::EnvFilter;

mod cli;

use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing()?;

    let args = Cli::parse();
    let config = SnpmConfig::from_env();

    match args.command {
        Command::Install { packages } => {
            let cwd = env::current_dir()?;
            let project = Project::discover(&cwd)?;
            let options = operations::InstallOptions {
                requested: packages,
            };
            operations::install(&config, &project, options).await?;
        }
        Command::Run { script, args } => {
            let cwd = env::current_dir()?;
            let project = Project::discover(&cwd)?;
            operations::run_script(&project, &script, &args)?;
        }
        Command::Remove { packages } => {
            let cwd = env::current_dir()?;
            let mut project = Project::discover(&cwd)?;
            operations::remove(&config, &mut project, packages).await?;
        }
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
