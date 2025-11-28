use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "snpm", about = "speedy node package manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Install {
        #[arg(long)]
        production: bool,
        #[arg(long = "frozen-lockfile", alias = "immutable")]
        frozen_lockfile: bool,
        #[arg(short = 'f', long = "force")]
        force: bool,
        packages: Vec<String>,
    },
    Add {
        #[arg(short = 'D', long = "dev")]
        dev: bool,
        #[arg(short = 'f', long = "force")]
        force: bool,
        packages: Vec<String>,
        /// Target a specific workspace broject by its package name
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    Remove {
        packages: Vec<String>,
    },
    Run {
        script: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Init,
    Upgrade {
        #[arg(long)]
        production: bool,
        #[arg(short = 'f', long = "force")]
        force: bool,
        packages: Vec<String>,
    },
    Outdated {
        #[arg(long)]
        production: bool,
    },
}
