use crate::Result;
use crate::resolve::{PackageId, ResolutionGraph};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(in crate::operations::install::workspace) fn rebuild_virtual_store_paths(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    let mut paths = BTreeMap::new();

    for id in graph.packages.keys() {
        paths.insert(id.clone(), virtual_package_location(virtual_store_dir, id));
    }

    Ok(paths)
}

pub(super) fn virtual_id_dir(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    let safe_name = id.name.replace('/', "+");
    virtual_store_dir.join(format!("{}@{}", safe_name, id.version))
}

pub(super) fn virtual_package_location(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    virtual_id_dir(virtual_store_dir, id)
        .join("node_modules")
        .join(&id.name)
}

pub(super) fn virtual_package_ready(package_location: &Path) -> bool {
    let is_real_dir = package_location
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());

    is_real_dir
        && fs::read_dir(package_location)
            .ok()
            .and_then(|mut entries| entries.next())
            .is_some()
}
