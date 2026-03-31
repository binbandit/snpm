use crate::{Result, SnpmConfig, SnpmError};

use std::fs;
use std::path::Path;

use super::types::{CleanOptions, CleanSummary};

pub fn analyze(config: &SnpmConfig, options: &CleanOptions) -> Result<CleanSummary> {
    let mut summary = CleanSummary::default();

    if options.packages {
        let packages_directory = config.packages_dir();
        if packages_directory.exists() {
            let (count, size) = count_directory_contents(&packages_directory)?;
            summary.packages_count = count;
            summary.packages_size = size;
        }
    }

    if options.metadata {
        let metadata_directory = config.metadata_dir();
        if metadata_directory.exists() {
            let (count, size) = count_directory_contents(&metadata_directory)?;
            summary.metadata_count = count;
            summary.metadata_size = size;
        }
    }

    if options.global {
        let global_directory = config.global_dir();
        let global_bin_directory = config.global_bin_dir();

        if global_directory.exists() {
            let (count, size) = count_directory_contents(&global_directory)?;
            summary.global_count += count;
            summary.global_size += size;
        }

        if global_bin_directory.exists() {
            let (count, size) = count_directory_contents(&global_bin_directory)?;
            summary.global_count += count;
            summary.global_size += size;
        }
    }

    Ok(summary)
}

fn count_directory_contents(path: &Path) -> Result<(usize, u64)> {
    if !path.exists() {
        return Ok((0, 0));
    }

    let entries = fs::read_dir(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let count = entries.filter_map(|entry| entry.ok()).count();
    let size = directory_size(path);

    Ok((count, size))
}

fn directory_size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| directory_size(&entry.path()))
        .sum()
}
