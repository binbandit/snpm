#[cfg(unix)]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use anyhow::{Context, Result};
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
        console::error(&format!("{error:#}"));
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let Cli { verbose, command } = Cli::parse();

    init_tracing()?;

    let mut config = SnpmConfig::from_env();

    if verbose || config.verbose || config.log_file.is_some() {
        config.verbose = true;

        let cwd = env::current_dir().context("failed to determine current directory")?;
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
        Command::Run(args) => commands::run::run(args, &config).await?,
        Command::Exec(args) => commands::exec::run(args, &config).await?,
        Command::Init(args) => commands::init::run(args).await?,
        Command::Dlx(args) => commands::dlx::run(args, &config).await?,
        Command::Upgrade(args) => commands::upgrade::run(args, &config).await?,
        Command::Outdated(args) => commands::outdated::run(args, &config).await?,
        Command::Licenses(args) => commands::licenses::run(args).await?,
        Command::Link(args) => commands::link::run(args, &config).await?,
        Command::List(args) => commands::list::run(args, &config).await?,
        Command::Login(args) => commands::login::run(args, &config).await?,
        Command::Logout(args) => commands::logout::run(args, &config).await?,
        Command::Config(args) => commands::config::run(args, &config).await?,
        Command::Pack(args) => commands::pack::run(args).await?,
        Command::Publish(args) => commands::publish::run(args, &config).await?,
        Command::Rebuild(args) => commands::rebuild::run(args, &config).await?,
        Command::Patch(args) => commands::patch::run(args, &config).await?,
        Command::Clean(args) => commands::clean::run(args, &config).await?,
        Command::Audit(args) => commands::audit::run(args, &config).await?,
        Command::Why(args) => commands::why::run(args).await?,
        Command::Store(args) => commands::store::run(args, &config).await?,
        Command::Unlink(args) => commands::unlink::run(args, &config).await?,
        Command::Completions(args) => commands::completions::run(args).await?,
        Command::Script(args) => {
            let mut iter = args.into_iter();
            let script = iter
                .next()
                .context("external subcommand must have a name")?;
            let extra_args: Vec<String> = iter.collect();
            let run_args = commands::run::RunArgs {
                script,
                recursive: false,
                filter: vec![],
                skip_install: false,
                args: extra_args,
            };
            commands::run::run(run_args, &config).await?
        }
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
