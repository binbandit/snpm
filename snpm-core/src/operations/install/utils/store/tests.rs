use super::check_store_cache;
use crate::config::SnpmConfig;
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage};
use crate::store::persist_store_residency_index;

use std::collections::BTreeMap;
use tempfile::tempdir;

fn make_config() -> SnpmConfig {
    SnpmConfig::for_tests()
}

#[test]
fn check_store_cache_uses_detected_package_root() {
    let dir = tempdir().unwrap();
    let packages_dir = dir.path().join("data/packages");
    let store_dir = packages_dir.join("@types_body-parser/1.19.6");
    let package_root = store_dir.join("body-parser");

    std::fs::create_dir_all(&package_root).unwrap();
    std::fs::write(package_root.join("package.json"), "{}").unwrap();
    std::fs::write(store_dir.join(".snpm_complete"), "").unwrap();

    let mut config = make_config();
    config.data_dir = dir.path().join("data");

    let id = PackageId {
        name: "@types/body-parser".to_string(),
        version: "1.19.6".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/types-body-parser.tgz".to_string(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
        bin: None,
    };
    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages: BTreeMap::from([(id.clone(), pkg)]),
    };

    let cache = check_store_cache(&config, &graph);
    assert!(cache.missing.is_empty());
    assert_eq!(cache.cached.get(&id), Some(&package_root));
}

#[test]
fn check_store_cache_uses_store_residency_index_when_present() {
    let dir = tempdir().unwrap();
    let packages_dir = dir.path().join("data/packages");
    let store_dir = packages_dir.join("@types_body-parser/1.19.6");
    let package_root = store_dir.join("package");

    std::fs::create_dir_all(&package_root).unwrap();
    std::fs::write(package_root.join("package.json"), "{}").unwrap();
    std::fs::write(store_dir.join(".snpm_complete"), "").unwrap();

    let mut config = make_config();
    config.data_dir = dir.path().join("data");

    let id = PackageId {
        name: "@types/body-parser".to_string(),
        version: "1.19.6".to_string(),
    };
    persist_store_residency_index(
        &config,
        &BTreeMap::from([(id.clone(), package_root.clone())]),
    )
    .unwrap();

    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/types-body-parser.tgz".to_string(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
        bin: None,
    };
    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages: BTreeMap::from([(id.clone(), pkg)]),
    };

    let cache = check_store_cache(&config, &graph);
    assert!(cache.missing.is_empty());
    assert_eq!(cache.cached.get(&id), Some(&package_root));
}
