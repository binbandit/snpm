use anyhow::Result;
use snpm_core::{console, operations::start_patch};

use super::print_header;
use super::project::discover_project;

pub(super) async fn run_edit(package: &str) -> Result<()> {
    print_header("patch edit");

    let project = discover_project()?;

    console::step(&format!("Preparing {} for patching...", package));

    let result = start_patch(&project, package)?;

    println!();
    println!(
        "Ready to patch {} @ {}",
        result.package_name, result.package_version
    );
    println!();
    println!("Edit the package at:");
    println!("  {}", result.edit_dir.display());
    println!();
    println!("When done, run:");
    println!("  snpm patch commit {}", result.edit_dir.display());
    println!();

    Ok(())
}
