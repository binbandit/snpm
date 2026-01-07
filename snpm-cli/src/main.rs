use anyhow::Result;
use clap::Parser;
use snpm_core::{SnpmConfig, console};
use std::{env, process};
use tracing_subscriber::EnvFilter;

mod cli;
mod commands;

use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        console::error(&format!("{error}"));
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let Cli { verbose, command } = Cli::parse();

    init_tracing()?;

    let mut config = SnpmConfig::from_env();

    if verbose || config.verbose || config.log_file.is_some() {
        config.verbose = true;

        let cwd = env::current_dir()?;
        let log_path = config
            .log_file
            .clone()
            .unwrap_or_else(|| cwd.join(".snpm.log"));

        console::init_logging(&log_path)?;
        console::verbose(&format!(
            "verbose logging enabled, writing to {}",
            log_path.display()
        ));
    }

    match command {
        Command::Install(args) => commands::install::run(args, &config).await?,
        Command::Add(args) => commands::add::run(args, &config).await?,
        Command::Remove(args) => commands::remove::run(args, &config).await?,
        Command::Run(args) => commands::run::run(args).await?,
        Command::Init(args) => commands::init::run(args).await?,
        Command::Dlx(args) => commands::dlx::run(args, &config).await?,
        Command::Upgrade(args) => commands::upgrade::run(args, &config).await?,
        Command::Outdated(args) => commands::outdated::run(args, &config).await?,
        Command::Login(args) => commands::login::run(args, &config).await?,
        Command::Logout(args) => commands::logout::run(args, &config).await?,
        Command::Config(args) => commands::config::run(args, &config).await?,
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
