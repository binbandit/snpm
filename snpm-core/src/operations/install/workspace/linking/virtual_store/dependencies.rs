use crate::linker::fs::{
    copy_dir, ensure_parent_dir, package_node_modules, symlink_dir_entry, symlink_is_correct,
};
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmError};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub(in crate::operations::install::workspace) fn link_store_dependencies(
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let packages: Vec<_> = graph.packages.iter().collect();

    packages.par_iter().try_for_each(|(id, package)| {
        let package_location =
            virtual_store_paths
                .get(id)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;
        let package_node_modules =
            package_node_modules(package_location, &id.name).ok_or_else(|| {
                SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                }
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

            fs::remove_file(&dep_link).ok();
            fs::remove_dir_all(&dep_link).ok();

            ensure_parent_dir(&dep_link)?;
            symlink_dir_entry(dep_target, &dep_link)
                .or_else(|_| copy_dir(dep_target, &dep_link))?;
        }

        Ok::<(), SnpmError>(())
    })
}
