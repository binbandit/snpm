#[cfg(unix)]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use anyhow::{Context, Result};
use clap::Parser;
use snpm_core::{SnpmConfig, console};
use std::ffi::OsString;
use std::path::Path;
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
    let args = rewrite_multicall_argv(env::args_os().collect());
    let Cli {
        verbose,
        command,
        frozen_lockfile,
        no_frozen_lockfile,
        prefer_frozen_lockfile,
    } = Cli::parse_from(args);
    commands::frozen::set_global_frozen_override(commands::frozen::frozen_override_from_cli(
        frozen_lockfile,
        no_frozen_lockfile,
        prefer_frozen_lockfile,
    ));

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
                filter_prod: vec![],
                skip_install: false,
                args: extra_args,
            };
            commands::run::run(run_args, &config).await?
        }
    }

    Ok(())
}

fn rewrite_multicall_argv(mut args: Vec<OsString>) -> Vec<OsString> {
    let Some(argv0) = args.first() else {
        return args;
    };

    let stem = Path::new(argv0)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("snpm")
        .to_ascii_lowercase();

    let subcommand = match stem.as_str() {
        "snpr" => Some("run"),
        "snpx" => Some("dlx"),
        "pnpx" => Some("dlx"),
        _ => None,
    };
    if let Some(subcommand) = subcommand {
        args[0] = OsString::from("snpm");
        if matches!(
            args.get(1).and_then(|arg| arg.to_str()),
            Some("--version") | Some("-V")
        ) {
            return args;
        }
        args.insert(1, OsString::from(subcommand));
    }

    args
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::rewrite_multicall_argv;
    use std::ffi::OsString;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn rewrites_snpr_to_run() {
        let rewritten = rewrite_multicall_argv(vec![
            OsString::from("/tmp/snpr"),
            OsString::from("check"),
            OsString::from("--skip-install"),
        ]);

        assert_eq!(
            strings(&rewritten),
            vec!["snpm", "run", "check", "--skip-install"]
        );
    }

    #[test]
    fn rewrites_snpx_to_dlx() {
        let rewritten = rewrite_multicall_argv(vec![
            OsString::from("/tmp/snpx"),
            OsString::from("cowsay"),
        ]);

        assert_eq!(strings(&rewritten), vec!["snpm", "dlx", "cowsay"]);
    }

    #[test]
    fn preserves_version_requests_for_multicall_aliases() {
        let rewritten = rewrite_multicall_argv(vec![
            OsString::from("/tmp/pnpx"),
            OsString::from("--version"),
        ]);

        assert_eq!(strings(&rewritten), vec!["snpm", "--version"]);
    }
}
