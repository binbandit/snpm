use anyhow::Result;
use snpm_core::{console, operations::remove_package_patch};

use super::print_header;
use super::project::discover_project;

pub(super) async fn run_remove(package: &str) -> Result<()> {
    print_header("patch remove");

    let project = discover_project()?;

    console::step(&format!("Removing patch for {}...", package));

    match remove_package_patch(&project, package)? {
        Some(path) => {
            println!();
            console::info(&format!("Removed patch for {}", package));
            println!("  Deleted: {}", path.display());
            println!();
            println!("Run `snpm install` to restore the original package");
            println!();
        }
        None => {
            println!();
            console::warn(&format!("No patch found for {}", package));
            println!();
        }
    }

    Ok(())
}
