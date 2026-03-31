use crate::{Result, SnpmConfig, SnpmError, console};

use std::fs;
use std::path::Path;

use super::super::install::manifest::parse_spec;

pub async fn remove_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let global_dir = config.global_dir();
    let global_bin_dir = config.global_bin_dir();

    for spec in &packages {
        let (name, _) = parse_spec(spec);
        remove_package(&global_dir, &global_bin_dir, &name)?;
        console::removed(&name);
    }

    Ok(())
}

fn remove_package(global_dir: &Path, global_bin_dir: &Path, package_name: &str) -> Result<()> {
    let package_dir = global_dir.join(package_name);
    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|source| SnpmError::WriteFile {
            path: package_dir.clone(),
            source,
        })?;
    }

    remove_package_bins(package_name, global_bin_dir)
}

fn remove_package_bins(package_name: &str, bin_dir: &Path) -> Result<()> {
    if !bin_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(bin_dir).map_err(|source| SnpmError::ReadFile {
        path: bin_dir.to_path_buf(),
        source,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_symlink() {
            continue;
        }

        if let Ok(target) = fs::read_link(&path)
            && target.to_string_lossy().contains(package_name)
        {
            fs::remove_file(&path).ok();
        }
    }

    Ok(())
}
