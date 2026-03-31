use crate::linker::fs::{ensure_parent_dir, link_dir};
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmConfig, SnpmError};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::paths::{virtual_id_dir, virtual_package_location, virtual_package_ready};

pub(in crate::operations::install::workspace) fn populate_virtual_store(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    let virtual_store_paths = Arc::new(Mutex::new(BTreeMap::new()));
    let packages: Vec<_> = graph.packages.iter().collect();

    packages.par_iter().try_for_each(|(id, _)| -> Result<()> {
        let virtual_id_dir = virtual_id_dir(virtual_store_dir, id);
        let package_location = virtual_package_location(virtual_store_dir, id);
        let marker_file = virtual_id_dir.join(".snpm_linked");

        let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

        if marker_file.is_file() && virtual_package_ready(&package_location) {
            record_virtual_store_path(&virtual_store_paths, id, package_location);
            return Ok(());
        }

        if marker_file.is_file() {
            fs::remove_file(&marker_file).ok();
        }

        fs::remove_file(&package_location).ok();
        fs::remove_dir_all(&package_location).ok();

        ensure_parent_dir(&package_location)?;
        link_dir(config, store_path, &package_location)?;

        fs::write(&marker_file, []).map_err(|source| SnpmError::WriteFile {
            path: marker_file,
            source,
        })?;

        record_virtual_store_path(&virtual_store_paths, id, package_location);
        Ok(())
    })?;

    let mutex = Arc::try_unwrap(virtual_store_paths).map_err(|_| SnpmError::Internal {
        reason: "virtual store paths Arc still has multiple owners".into(),
    })?;

    Ok(mutex
        .into_inner()
        .unwrap_or_else(|error| error.into_inner()))
}

fn record_virtual_store_path(
    virtual_store_paths: &Arc<Mutex<BTreeMap<PackageId, PathBuf>>>,
    id: &PackageId,
    package_location: PathBuf,
) {
    virtual_store_paths
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(id.clone(), package_location);
}
