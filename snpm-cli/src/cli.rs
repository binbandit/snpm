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
    #[arg(long = "frozen-lockfile", global = true, conflicts_with_all = ["no_frozen_lockfile", "prefer_frozen_lockfile"])]
    pub frozen_lockfile: bool,

    #[arg(
        long = "no-frozen-lockfile",
        global = true,
        conflicts_with_all = ["frozen_lockfile", "prefer_frozen_lockfile"]
    )]
    pub no_frozen_lockfile: bool,

    #[arg(
        long = "prefer-frozen-lockfile",
        global = true,
        conflicts_with_all = ["frozen_lockfile", "no_frozen_lockfile"]
    )]
    pub prefer_frozen_lockfile: bool,

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
    /// List licenses of installed packages
    Licenses(commands::licenses::LicensesArgs),
    /// Link a package globally or into the current project
    Link(commands::link::LinkArgs),
    /// List installed packages
    List(commands::list::ListArgs),
    /// Authenticate with a registry
    Login(commands::login::LoginArgs),
    /// Remove stored registry credentials
    Logout(commands::logout::LogoutArgs),
    /// Show the resolved configuration
    Config(commands::config::ConfigArgs),
    /// Create a tarball from the current package
    Pack(commands::pack::PackArgs),
    /// Publish a package to the registry
    Publish(commands::publish::PublishArgs),
    /// Rebuild native modules
    Rebuild(commands::rebuild::RebuildArgs),
    /// Patch packages to fix bugs or customize behavior
    Patch(commands::patch::PatchArgs),
    /// Remove cached packages and metadata to free disk space
    Clean(commands::clean::CleanArgs),
    /// Scan dependencies for security vulnerabilities
    Audit(commands::audit::AuditArgs),
    /// Explain why a dependency is installed
    Why(commands::why::WhyArgs),
    /// Manage the package store
    Store(commands::store::StoreArgs),
    /// Remove a linked package
    Unlink(commands::unlink::UnlinkArgs),

    /// Generate shell completions
    #[command(hide = true)]
    Completions(commands::completions::CompletionsArgs),

    /// Run a package.json script by name (fallback for unknown subcommands)
    #[command(external_subcommand)]
    Script(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_global_frozen_lockfile_before_subcommand() {
        let cli = Cli::try_parse_from(["snpm", "--frozen-lockfile", "install"]).unwrap();

        assert!(cli.frozen_lockfile);
        assert!(!cli.no_frozen_lockfile);
        assert!(!cli.prefer_frozen_lockfile);
        match cli.command {
            Command::Install(args) => {
                assert!(args.frozen_lockfile);
                assert!(!args.no_frozen_lockfile);
                assert!(!args.prefer_frozen_lockfile);
                assert!(!args.fix_lockfile);
            }
            other => panic!("expected install command, got {other:?}"),
        }
    }

    #[test]
    fn parses_install_specific_fix_lockfile_after_subcommand() {
        let cli = Cli::try_parse_from(["snpm", "install", "--fix-lockfile"]).unwrap();

        assert!(!cli.frozen_lockfile);
        assert!(!cli.no_frozen_lockfile);
        assert!(!cli.prefer_frozen_lockfile);
        match cli.command {
            Command::Install(args) => {
                assert!(!args.frozen_lockfile);
                assert!(!args.no_frozen_lockfile);
                assert!(!args.prefer_frozen_lockfile);
                assert!(args.fix_lockfile);
            }
            other => panic!("expected install command, got {other:?}"),
        }
    }
}
