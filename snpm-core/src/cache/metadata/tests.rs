use super::freshness::is_fresh;
use super::{load_metadata, load_metadata_with_offline, save_metadata};
use crate::config::{AuthScheme, HoistingMode, LinkBackend, OfflineMode, SnpmConfig};
use crate::registry::{RegistryDist, RegistryPackage, RegistryVersion};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tempfile::tempdir;

fn make_config(data_dir: PathBuf) -> SnpmConfig {
    SnpmConfig {
        cache_dir: data_dir.join("cache"),
        data_dir,
        allow_scripts: BTreeSet::new(),
        min_package_age_days: None,
        min_package_cache_age_days: Some(7),
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

fn make_package() -> RegistryPackage {
    let mut versions = BTreeMap::new();
    versions.insert(
        "1.0.0".to_string(),
        RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies: None,
            bundle_dependencies: None,
            dist: RegistryDist {
                tarball: "https://example.com/pkg.tgz".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: None,
        },
    );
    let mut dist_tags = BTreeMap::new();
    dist_tags.insert("latest".to_string(), "1.0.0".to_string());

    RegistryPackage {
        versions,
        time: BTreeMap::new(),
        dist_tags,
    }
}

#[test]
fn save_and_load_metadata_roundtrip() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    let package = make_package();

    save_metadata(&config, "test-pkg", &package).unwrap();

    let loaded = load_metadata_with_offline(&config, "test-pkg", OfflineMode::PreferOffline);
    assert!(loaded.is_some());

    let loaded = loaded.unwrap();
    assert!(loaded.versions.contains_key("1.0.0"));
    assert_eq!(
        loaded.dist_tags.get("latest").map(String::as_str),
        Some("1.0.0")
    );
}

#[test]
fn load_metadata_returns_none_when_not_cached() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());

    let loaded = load_metadata(&config, "nonexistent-pkg");
    assert!(loaded.is_none());
}

#[test]
fn is_fresh_returns_true_for_recent_file() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    let file = dir.path().join("recent");
    std::fs::write(&file, "data").unwrap();

    assert!(is_fresh(&config, &file));
}

#[test]
fn is_fresh_returns_false_when_no_cache_age_configured() {
    let dir = tempdir().unwrap();
    let mut config = make_config(dir.path().to_path_buf());
    config.min_package_cache_age_days = None;
    let file = dir.path().join("file");
    std::fs::write(&file, "data").unwrap();

    assert!(!is_fresh(&config, &file));
}

#[test]
fn is_fresh_returns_false_for_nonexistent_file() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    assert!(!is_fresh(&config, &dir.path().join("nonexistent")));
}
