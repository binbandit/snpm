use super::check_store_cache;
use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tempfile::tempdir;

fn make_config() -> SnpmConfig {
    SnpmConfig {
        cache_dir: PathBuf::from("/tmp/cache"),
        data_dir: PathBuf::from("/tmp/data"),
        allow_scripts: BTreeSet::new(),
        min_package_age_days: None,
        min_package_cache_age_days: None,
        default_registry: "https://registry.npmjs.org".to_string(),
        scoped_registries: BTreeMap::new(),
        registry_auth: BTreeMap::new(),
        default_registry_auth_token: None,
        default_registry_auth_scheme: AuthScheme::Bearer,
        registry_auth_schemes: BTreeMap::new(),
        hoisting: HoistingMode::SingleVersion,
        link_backend: LinkBackend::Auto,
        strict_peers: false,
        frozen_lockfile_default: false,
        always_auth: false,
        registry_concurrency: 64,
        verbose: false,
        log_file: None,
    }
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
