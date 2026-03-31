use anyhow::{Context, Result};
use snpm_core::Project;
use std::env;

pub(super) fn discover_project() -> Result<Project> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    Ok(Project::discover(&cwd)?)
}
