use anyhow::Result;
use clap::Args;
use snpm_core::{SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct DlxArgs {
    /// Package to download and run (e.g. "cowsay" or "cowsay@latest")
    pub package: String,
    /// Arguments to pass to the package's binary
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: DlxArgs, config: &SnpmConfig) -> Result<()> {
    console::header(&format!("dlx {}", args.package), env!("CARGO_PKG_VERSION"));
    operations::dlx(config, args.package, args.args).await?;
    Ok(())
}
