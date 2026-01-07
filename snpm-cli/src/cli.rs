use crate::commands;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "snpm", about = "speedy node package manager", version)]
pub struct Cli {
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Install(commands::install::InstallArgs),
    Add(commands::add::AddArgs),
    Remove(commands::remove::RemoveArgs),
    Run(commands::run::RunArgs),
    Init(commands::init::InitArgs),
    Dlx(commands::dlx::DlxArgs),
    Upgrade(commands::upgrade::UpgradeArgs),
    Outdated(commands::outdated::OutdatedArgs),
    Login(commands::login::LoginArgs),
    Logout(commands::logout::LogoutArgs),
    Config(commands::config::ConfigArgs),
}
