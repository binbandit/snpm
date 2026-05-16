mod alias;
mod current;
mod default;
mod env;
mod exec;
mod install;
mod list;
mod ls_remote;
mod run;
mod unalias;
mod uninstall;
mod use_cmd;
mod which;

use anyhow::Result;
use clap::{Args, Subcommand};
use snpm_core::SnpmConfig;

#[derive(Args, Debug)]
#[command(arg_required_else_help = true)]
pub struct NodeArgs {
    #[command(subcommand)]
    pub command: NodeCommand,
}

#[derive(Subcommand, Debug)]
pub enum NodeCommand {
    /// Download and install a Node.js version
    #[command(alias = "i")]
    Install(install::InstallArgs),
    /// Remove an installed Node.js version
    #[command(alias = "rm")]
    Uninstall(uninstall::UninstallArgs),
    /// Activate a Node.js version for the current shell or set the default
    Use(use_cmd::UseArgs),
    /// List installed Node.js versions
    #[command(alias = "ls")]
    List(list::ListArgs),
    /// List versions available from nodejs.org
    #[command(alias = "ls-remote", alias = "remote")]
    ListRemote(ls_remote::LsRemoteArgs),
    /// Show the currently active Node.js version
    Current(current::CurrentArgs),
    /// Print the path to a Node.js binary
    Which(which::WhichArgs),
    /// Manage named aliases (default, work, lts/iron, ...)
    Alias(alias::AliasArgs),
    /// Remove a Node.js alias
    Unalias(unalias::UnaliasArgs),
    /// Shortcut for `alias default <version>`
    Default(default::DefaultArgs),
    /// Run a one-off command with a specific Node.js version
    Exec(exec::ExecArgs),
    /// Run a package.json script with a specific Node.js version
    Run(run::RunArgs),
    /// Print shell integration that auto-switches Node versions
    Env(env::EnvArgs),
}

pub async fn run(args: NodeArgs, config: &SnpmConfig) -> Result<()> {
    match args.command {
        NodeCommand::Install(args) => install::run(args, config).await,
        NodeCommand::Uninstall(args) => uninstall::run(args, config),
        NodeCommand::Use(args) => use_cmd::run(args, config).await,
        NodeCommand::List(args) => list::run(args, config),
        NodeCommand::ListRemote(args) => ls_remote::run(args, config).await,
        NodeCommand::Current(args) => current::run(args, config),
        NodeCommand::Which(args) => which::run(args, config).await,
        NodeCommand::Alias(args) => alias::run(args, config).await,
        NodeCommand::Unalias(args) => unalias::run(args, config),
        NodeCommand::Default(args) => default::run(args, config).await,
        NodeCommand::Exec(args) => exec::run(args, config).await,
        NodeCommand::Run(args) => run::run(args, config).await,
        NodeCommand::Env(args) => env::run(args),
    }
}
