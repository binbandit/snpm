use crate::{Result, SnpmConfig, SnpmError, console};
use std::fs;
use std::path::Path;

pub struct StoreStatus {
    pub packages_count: usize,
    pub packages_size: u64,
    pub metadata_count: usize,
    pub metadata_size: u64,
    pub store_path: String,
}

pub fn status(config: &SnpmConfig) -> Result<StoreStatus> {
    let packages_dir = config.packages_dir();
    let metadata_dir = config.metadata_dir();

    let (packages_count, packages_size) = if packages_dir.exists() {
        count_entries_and_size(&packages_dir)?
    } else {
        (0, 0)
    };

    let (metadata_count, metadata_size) = if metadata_dir.exists() {
        count_entries_and_size(&metadata_dir)?
    } else {
        (0, 0)
    };

    Ok(StoreStatus {
        packages_count,
        packages_size,
        metadata_count,
        metadata_size,
        store_path: packages_dir.display().to_string(),
    })
}

pub fn prune(config: &SnpmConfig, dry_run: bool) -> Result<usize> {
    let packages_dir = config.packages_dir();
    if !packages_dir.exists() {
        return Ok(0);
    }

    // Find packages not accessed recently by checking for stale .snpm_complete markers
    let mut pruned = 0;

    let entries = fs::read_dir(&packages_dir).map_err(|source| SnpmError::ReadFile {
        path: packages_dir.clone(),
        source,
    })?;

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        // Each package name dir contains version dirs
        let version_entries = match fs::read_dir(&entry_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for version_entry in version_entries.flatten() {
            let version_path = version_entry.path();
            if !version_path.is_dir() {
                continue;
            }

            let marker = version_path.join(".snpm_complete");
            if !marker.is_file() {
                // Incomplete extraction — safe to remove
                let name = entry_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let version = version_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();

                if dry_run {
                    console::info(&format!(
                        "would remove incomplete package: {}@{}",
                        name, version
                    ));
                } else {
                    console::verbose(&format!(
                        "removing incomplete package: {}@{}",
                        name, version
                    ));
                    fs::remove_dir_all(&version_path).ok();
                }
                pruned += 1;
            }
        }

        // Clean up empty package name directories
        if !dry_run
            && let Ok(mut remaining) = fs::read_dir(&entry_path)
            && remaining.next().is_none()
        {
            fs::remove_dir(&entry_path).ok();
        }
    }

    Ok(pruned)
}

pub fn path(config: &SnpmConfig) -> String {
    config.packages_dir().display().to_string()
}

fn count_entries_and_size(path: &Path) -> Result<(usize, u64)> {
    let entries = fs::read_dir(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let count = entries.filter_map(|e| e.ok()).count();
    let size = dir_size(path);

    Ok((count, size))
}

fn dir_size(path: &Path) -> u64 {
    if path.is_file() {
        return path.metadata().map(|m| m.len()).unwrap_or(0);
    }

    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };

    entries
        .filter_map(|e| e.ok())
        .map(|e| dir_size(&e.path()))
        .sum()
}
