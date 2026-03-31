use crate::version;

use std::process::ExitCode;

pub(super) fn run_list() -> anyhow::Result<ExitCode> {
    let versions = version::list_cached_versions()?;

    if versions.is_empty() {
        println!("No snpm versions cached.");
    } else {
        println!("Cached snpm versions:");
        for version in versions {
            println!("  {}", version);
        }
    }

    Ok(ExitCode::SUCCESS)
}
