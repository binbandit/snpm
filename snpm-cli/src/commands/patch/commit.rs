use anyhow::Result;
use snpm_core::{console, operations::commit_patch};

use std::path::Path;

use super::print_header;
use super::project::discover_project;

pub(super) async fn run_commit(path: &Path) -> Result<()> {
    print_header("patch commit");

    let project = discover_project()?;

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    console::step("Creating patch from modifications...");

    let result = commit_patch(&project, path)?;

    println!();
    console::info(&format!(
        "Created patch for {} @ {}",
        result.package_name, result.package_version
    ));
    println!();
    println!("Patch saved to:");
    println!("  {}", result.patch_path.display());
    println!();
    println!("The patch will be applied automatically on `snpm install`");
    println!();

    Ok(())
}
