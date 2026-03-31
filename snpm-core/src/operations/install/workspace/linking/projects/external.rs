use crate::linker::{bins::link_bins, fs::symlink_dir_entry};
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmError};

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn link_external_deps(
    manifest_deps: &BTreeMap<String, String>,
    workspace_deps: &BTreeSet<String>,
    graph: &ResolutionGraph,
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    node_modules: &Path,
) -> Result<()> {
    for (name, value) in manifest_deps {
        if value.starts_with("workspace:") || workspace_deps.contains(name) {
            continue;
        }

        if let Some(root_dep) = graph.root.dependencies.get(name) {
            let target = virtual_store_paths.get(&root_dep.resolved).ok_or_else(|| {
                SnpmError::GraphMissing {
                    name: root_dep.resolved.name.clone(),
                    version: root_dep.resolved.version.clone(),
                }
            })?;

            let destination = node_modules.join(name);
            create_symlink(target, &destination)?;
            link_bins(&destination, node_modules, name).ok();
        }
    }

    Ok(())
}

fn create_symlink(target: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).ok();
    }

    if destination.exists() || destination.symlink_metadata().is_ok() {
        if destination.is_dir() {
            fs::remove_dir_all(destination).ok();
        } else {
            fs::remove_file(destination).ok();
        }
    }

    symlink_dir_entry(target, destination).map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}
