use crate::{Result, SnpmConfig, SnpmError, console};
use std::fs;
use std::path::Path;

#[derive(Debug, Default)]
pub struct CleanSummary {
    pub packages_count: usize,
    pub packages_size: u64,
    pub metadata_count: usize,
    pub metadata_size: u64,
    pub global_count: usize,
    pub global_size: u64,
}

impl CleanSummary {
    pub fn total_size(&self) -> u64 {
        self.packages_size + self.metadata_size + self.global_size
    }

    pub fn total_count(&self) -> usize {
        self.packages_count + self.metadata_count + self.global_count
    }

    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }
}

#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub packages: bool,
    pub metadata: bool,
    pub global: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            packages: true,
            metadata: true,
            global: false,
        }
    }
}

impl CleanOptions {
    pub fn all() -> Self {
        Self {
            packages: true,
            metadata: true,
            global: true,
        }
    }
}

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

pub fn format_bytes(bytes: u64) -> String {
    const KILOBYTE: u64 = 1024;
    const MEGABYTE: u64 = KILOBYTE * 1024;
    const GIGABYTE: u64 = MEGABYTE * 1024;

    if bytes >= GIGABYTE {
        format!("{:.2} GB", bytes as f64 / GIGABYTE as f64)
    } else if bytes >= MEGABYTE {
        format!("{:.2} MB", bytes as f64 / MEGABYTE as f64)
    } else if bytes >= KILOBYTE {
        format!("{:.2} KB", bytes as f64 / KILOBYTE as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
