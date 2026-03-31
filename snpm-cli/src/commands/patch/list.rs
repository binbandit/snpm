use anyhow::Result;
use snpm_core::operations::list_project_patches;

use super::print_header;
use super::print_no_patch_hint;
use super::project::discover_project;

pub(super) async fn run_list() -> Result<()> {
    print_header("patch list");

    let project = discover_project()?;
    let patches = list_project_patches(&project)?;

    if patches.is_empty() {
        println!("No patches in this project");
        println!();
        print_no_patch_hint();
        return Ok(());
    }

    println!("Patches ({}):", patches.len());
    println!();

    for patch in patches {
        println!("  {} @ {}", patch.package_name, patch.package_version);
        println!("    {}", patch.patch_path.display());
    }

    println!();

    Ok(())
}
