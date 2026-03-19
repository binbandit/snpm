use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct InitArgs;

pub async fn run(_args: InitArgs) -> Result<()> {
    console::header("init", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir().context("failed to determine current directory")?;
    operations::init(&cwd)?;

    Ok(())
}
