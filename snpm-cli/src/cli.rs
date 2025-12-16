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
    Install {
        #[arg(long)]
        production: bool,
        #[arg(long = "frozen-lockfile", alias = "immutable")]
        frozen_lockfile: bool,
        #[arg(short = 'f', long = "force")]
        force: bool,
        packages: Vec<String>,
        /// Target a specific workspace broject by its package name
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    Add {
        #[arg(short = 'D', long = "dev")]
        dev: bool,
        #[arg(short = 'f', long = "force")]
        force: bool,
        packages: Vec<String>,
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    Remove {
        packages: Vec<String>,
    },
    Run {
        /// Script name, e.g. "test"
        script: String,
        /// Run the script in all workspace projects
        #[arg(short = 'r', long = "recursive")]
        recursive: bool,
        /// Filter workspace projects by name (glob patterns like "app-*" are supported)
        #[arg(long = "filter")]
        filter: Vec<String>,
        /// Extra arguments passed to the script (use `--` to separate)
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Init,
    Dlx {
        /// Package to download and run (e.g. "cowsay" or "cowsay@latest")
        package: String,
        /// Arguments to pass to the package's binary
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
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
    Login {
        /// Registry URL to store credentials for. Defaults to the current default registry.
        #[arg(long)]
        registry: Option<String>,
        /// Auth token to save. If omitted, snpm will prompt for it.
        #[arg(long)]
        token: Option<String>,
        /// Associate a scope with the registry (e.g., @myorg)
        #[arg(long)]
        scope: Option<String>,
        /// Open browser for web-based authentication
        #[arg(long)]
        web: bool,
    },
    Logout {
        /// Registry URL to remove credentials for. Defaults to the current default registry.
        #[arg(long)]
        registry: Option<String>,
        /// Remove credentials for a specific scope (e.g., @myorg)
        #[arg(long)]
        scope: Option<String>,
    },
}
