use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmConfig, SnpmError};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::fs::{
    copy_dir, ensure_parent_dir, link_dir, package_node_modules, symlink_dir_entry,
    symlink_is_correct,
};

pub(super) fn populate_virtual_store(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let packages: Vec<_> = graph.packages.iter().collect();
    let results: Vec<Result<(PackageId, PathBuf)>> = packages
        .par_iter()
        .map(|(id, _package)| -> Result<(PackageId, PathBuf)> {
            let safe_name = id.name.replace('/', "+");
            let virtual_id_dir = virtual_store_dir.join(format!("{}@{}", safe_name, id.version));
            let package_location = virtual_id_dir.join("node_modules").join(&id.name);

            let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
                name: id.name.clone(),
                version: id.version.clone(),
            })?;

            if let Ok(meta) = package_location.symlink_metadata() {
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    return Ok(((*id).clone(), package_location));
                }
                std::fs::remove_file(&package_location).ok();
                std::fs::remove_dir_all(&package_location).ok();
            }

            ensure_parent_dir(&package_location)?;
            link_dir(config, store_path, &package_location)?;

            Ok(((*id).clone(), package_location))
        })
        .collect();

    let mut map = BTreeMap::new();
    for result in results {
        let (id, path) = result?;
        map.insert(id, path);
    }

    Ok(Arc::new(map))
}

pub(super) fn link_virtual_dependencies(
    virtual_store_paths: &Arc<BTreeMap<PackageId, PathBuf>>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let packages: Vec<_> = graph.packages.iter().collect();

    packages
        .par_iter()
        .try_for_each(|(id, package)| -> Result<()> {
            let package_location =
                virtual_store_paths
                    .get(id)
                    .ok_or_else(|| SnpmError::GraphMissing {
                        name: id.name.clone(),
                        version: id.version.clone(),
                    })?;
            let package_node_modules = package_node_modules(package_location, &id.name)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;

            for (dep_name, dep_id) in &package.dependencies {
                let dep_target =
                    virtual_store_paths
                        .get(dep_id)
                        .ok_or_else(|| SnpmError::GraphMissing {
                            name: dep_id.name.clone(),
                            version: dep_id.version.clone(),
                        })?;
                let dep_link = package_node_modules.join(dep_name);

                if symlink_is_correct(&dep_link, dep_target) {
                    continue;
                }

                std::fs::remove_file(&dep_link).ok();
                std::fs::remove_dir_all(&dep_link).ok();

                ensure_parent_dir(&dep_link)?;
                symlink_dir_entry(dep_target, &dep_link)
                    .or_else(|_| copy_dir(dep_target, &dep_link))?;
            }

            Ok(())
        })
}
