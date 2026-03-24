use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct PackArgs {}

pub async fn run(_args: PackArgs) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    let result = operations::pack(&project, &cwd)?;

    console::info(&format!(
        "Packed {} files into {} ({})",
        result.file_count,
        result.tarball_path.display(),
        operations::format_bytes(result.size)
    ));

    Ok(())
}
