use super::super::types::CacheCheckResult;
use crate::resolve::{ResolutionGraph, ResolvedPackage};
use crate::store::PACKAGE_METADATA_FILE;

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

pub fn check_store_cache(config: &crate::SnpmConfig, graph: &ResolutionGraph) -> CacheCheckResult {
    use rayon::prelude::*;

    let base = config.packages_dir();
    let store_index = crate::store::load_store_residency_index_lossy(config);
    let packages: Vec<_> = graph.packages.values().collect();
    let results: Vec<_> = packages
        .par_iter()
        .map(|package| cache_lookup(&base, store_index.as_ref(), package))
        .collect();

    let mut cached = BTreeMap::new();
    let mut missing = Vec::new();

    for (hit, miss) in results {
        if let Some((id, path)) = hit {
            cached.insert(id, path);
        }
        if let Some(package) = miss {
            missing.push(package);
        }
    }

    CacheCheckResult { cached, missing }
}

fn cache_lookup(
    base: &std::path::Path,
    store_index: Option<&crate::store::StoreResidencyIndexView>,
    package: &ResolvedPackage,
) -> (
    Option<(crate::resolve::PackageId, PathBuf)>,
    Option<ResolvedPackage>,
) {
    let name_dir = package.id.name.replace('/', "_");
    let package_directory = base.join(&name_dir).join(&package.id.version);
    let marker = package_directory.join(".snpm_complete");

    if marker.is_file()
        && let Some(root) = store_index.and_then(|index| index.resolve_root(base, &package.id))
        && cached_root_ready(&root)
    {
        return (Some((package.id.clone(), root)), None);
    }

    if marker.is_file() {
        let root = crate::store::package_root_dir(&package_directory);
        (Some((package.id.clone(), root)), None)
    } else {
        (None, Some(package.clone()))
    }
}

fn cached_root_ready(root: &Path) -> bool {
    root.is_dir()
        && (root.join(PACKAGE_METADATA_FILE).is_file() || root.join("package.json").is_file())
}
