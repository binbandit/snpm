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

    let _override = SnpmNodeOverride::set(bin_dir.to_string_lossy().to_string());
    operations::run_script(&project, &args.script, &args.args)?;
    Ok(())
}

struct SnpmNodeOverride {
    prior: Option<std::ffi::OsString>,
}

impl SnpmNodeOverride {
    fn set(value: String) -> Self {
        let prior = env::var_os("SNPM_NODE_BIN_OVERRIDE");
        unsafe { env::set_var("SNPM_NODE_BIN_OVERRIDE", value) };
        Self { prior }
    }
}

impl Drop for SnpmNodeOverride {
    fn drop(&mut self) {
        match &self.prior {
            Some(value) => unsafe { env::set_var("SNPM_NODE_BIN_OVERRIDE", value) },
            None => unsafe { env::remove_var("SNPM_NODE_BIN_OVERRIDE") },
        }
    }
}
