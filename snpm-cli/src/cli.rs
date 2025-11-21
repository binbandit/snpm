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
        packages: Vec<String>,
    },
    Run {
        script: String,
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}
