use crate::{Result, SnpmConfig, SnpmError, console};

use std::fs;
use std::path::Path;

use super::scan::analyze;
use super::types::{CleanOptions, CleanSummary};

pub fn execute(config: &SnpmConfig, options: &CleanOptions) -> Result<CleanSummary> {
    let summary = analyze(config, options)?;

    if options.packages {
        let packages_directory = config.packages_dir();
        if packages_directory.exists() {
            console::step("Removing cached packages...");
            remove_directory_contents(&packages_directory)?;
        }
    }

    if options.metadata {
        let metadata_directory = config.metadata_dir();
        if metadata_directory.exists() {
            console::step("Removing registry metadata cache...");
            remove_directory_contents(&metadata_directory)?;
        }
    }

    if options.global {
        let global_directory = config.global_dir();
        let global_bin_directory = config.global_bin_dir();

        if global_directory.exists() {
            console::step("Removing global packages...");
            remove_directory_contents(&global_directory)?;
        }

        if global_bin_directory.exists() {
            console::step("Removing global binaries...");
            remove_directory_contents(&global_bin_directory)?;
        }
    }

    Ok(summary)
}

fn remove_directory_contents(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            fs::remove_dir_all(&entry_path).map_err(|source| SnpmError::WriteFile {
                path: entry_path,
                source,
            })?;
        } else {
            fs::remove_file(&entry_path).map_err(|source| SnpmError::WriteFile {
                path: entry_path,
                source,
            })?;
        }
    }

    Ok(())
}
