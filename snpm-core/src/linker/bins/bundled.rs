use super::manifest::link_bins_from_bundled_pkg;
use crate::Result;
use crate::resolve::{PackageId, ResolutionGraph};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn link_bundled_bins_recursive(
    graph: &ResolutionGraph,
    linked: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    for (id, destination) in linked {
        if let Some(package) = graph.packages.get(id)
            && let Some(bundled) = &package.bundled_dependencies
            && !bundled.is_empty()
        {
            link_bundled_bins(destination)?;
        }
    }

    Ok(())
}

fn link_bundled_bins(pkg_dest: &Path) -> Result<()> {
    let bundled_modules = pkg_dest.join("node_modules");
    if !bundled_modules.is_dir() {
        return Ok(());
    }

    let bin_dir = bundled_modules.join(".bin");
    let Ok(entries) = fs::read_dir(&bundled_modules) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }

        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }

        if name.starts_with('@') {
            link_scoped_bundled_bins(&path, &bin_dir, &name)?;
        } else {
            link_bins_from_bundled_pkg(&path, &bin_dir, &name)?;
        }
    }

    Ok(())
}

fn link_scoped_bundled_bins(scope_path: &Path, bin_dir: &Path, scope_name: &str) -> Result<()> {
    let Ok(entries) = fs::read_dir(scope_path) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let package_path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(package_name) = file_name.to_str() else {
            continue;
        };

        let full_name = format!("{scope_name}/{package_name}");
        link_bins_from_bundled_pkg(&package_path, bin_dir, &full_name)?;
    }

    Ok(())
}
