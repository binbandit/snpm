use anyhow::{Context, Result};
use clap::Args;
use snpm_core::node::exec as node_exec;
use snpm_core::{Project, SnpmConfig, operations};

use std::env;

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Node.js version to use for the script
    pub version: String,
    /// package.json script name
    pub script: String,
    /// Extra arguments forwarded to the script
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub async fn run(args: RunArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    let (_, summary) = node_exec::ensure_installed_for_spec(config, &args.version).await?;
    let bin_dir = config.node_version_bin_dir(&summary.version);

    // Pass the Node bin dir explicitly instead of mutating process env:
    // env::set_var is unsound under the multi-threaded runtime.
    operations::run_script_with_node(&project, &args.script, &args.args, Some(&bin_dir))?;
    Ok(())
}
