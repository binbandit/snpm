use crate::resolve::{PackageId, ResolutionGraph, RootDependency};
use crate::{Result, SnpmError};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::bins::link_bins;
use super::fs::{copy_dir, ensure_parent_dir, symlink_dir_entry, symlink_is_correct};

pub(super) fn link_root_dependencies(
    root_deps: &[(&String, &RootDependency)],
    virtual_store_paths: &Arc<BTreeMap<PackageId, PathBuf>>,
    root_node_modules: &Path,
) -> Result<()> {
    root_deps
        .par_iter()
        .try_for_each(|(name, dep)| -> Result<()> {
            let id = &dep.resolved;
            let target = virtual_store_paths
                .get(id)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;

            let dest = root_node_modules.join(name);

            if symlink_is_correct(&dest, target) {
                return Ok(());
            }

            std::fs::remove_file(&dest).ok();
            std::fs::remove_dir_all(&dest).ok();

            if name.contains('/') {
                ensure_parent_dir(&dest)?;
            }

            symlink_dir_entry(target, &dest).or_else(|_| copy_dir(target, &dest))?;
            Ok(())
        })
}

pub(super) fn link_root_bins(
    root_deps: &[(&String, &RootDependency)],
    root_node_modules: &Path,
    graph: &ResolutionGraph,
) -> Result<()> {
    root_deps.par_iter().for_each(|(name, dep)| {
        if let Some(pkg) = graph.packages.get(&dep.resolved)
            && !pkg.has_bin
        {
            return;
        }

        let dest = root_node_modules.join(name);
        if let Err(error) = link_bins(&dest, root_node_modules, name) {
            crate::console::warn(&format!("failed to link bins for {}: {}", name, error));
        }
    });

    Ok(())
}
