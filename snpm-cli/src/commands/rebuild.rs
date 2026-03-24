use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct RebuildArgs {}

pub async fn run(_args: RebuildArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;
    let workspace = Workspace::discover(&project.root)?;

    let rebuilt = operations::rebuild(config, workspace.as_ref(), &project.root)?;

    if rebuilt == 0 {
        println!("Nothing to rebuild.");
    } else {
        console::info(&format!("Rebuilt {} packages.", rebuilt));
    }

    Ok(())
}
