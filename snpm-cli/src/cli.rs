use crate::commands;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "snpm",
    about = "speedy node package manager",
    version,
    color = clap::ColorChoice::Auto
)]
pub struct Cli {
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install dependencies for a project or workspace
    Install(commands::install::InstallArgs),
    /// Add packages to dependencies (or devDependencies with -D)
    Add(commands::add::AddArgs),
    /// Remove packages from dependencies
    Remove(commands::remove::RemoveArgs),
    /// Run a package.json script
    Run(commands::run::RunArgs),
    /// Execute a command with node_modules/.bin in PATH
    Exec(commands::exec::ExecArgs),
    /// Create a new package.json
    Init(commands::init::InitArgs),
    /// Download and run a package without installing
    Dlx(commands::dlx::DlxArgs),
    /// Upgrade dependencies and refresh the lockfile
    Upgrade(commands::upgrade::UpgradeArgs),
    /// Check for outdated dependencies
    Outdated(commands::outdated::OutdatedArgs),
    /// List installed packages
    List(commands::list::ListArgs),
    /// Authenticate with a registry
    Login(commands::login::LoginArgs),
    /// Remove stored registry credentials
    Logout(commands::logout::LogoutArgs),
    /// Show the resolved configuration
    Config(commands::config::ConfigArgs),
    /// Patch packages to fix bugs or customize behavior
    Patch(commands::patch::PatchArgs),
    /// Remove cached packages and metadata to free disk space
    Clean(commands::clean::CleanArgs),
    /// Scan dependencies for security vulnerabilities
    Audit(commands::audit::AuditArgs),
}
