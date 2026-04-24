use super::freshness::is_fresh;
use super::{load_metadata, load_metadata_with_offline, save_metadata, save_metadata_with_headers};
use crate::cache::CachedHeaders;
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
        disable_global_virtual_store_for_packages: BTreeSet::new(),
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
fn save_metadata_with_headers_roundtrip() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    let package = make_package();
    let headers = CachedHeaders {
        etag: Some("\"abc123\"".to_string()),
        last_modified: Some("Thu, 01 Jan 2026 00:00:00 GMT".to_string()),
    };

    save_metadata_with_headers(&config, "test-pkg", &package, Some(&headers)).unwrap();

    let loaded = crate::cache::load_cached_headers(&config, "test-pkg").unwrap();
    assert_eq!(loaded.etag.as_deref(), Some("\"abc123\""));
    assert_eq!(
        loaded.last_modified.as_deref(),
        Some("Thu, 01 Jan 2026 00:00:00 GMT")
    );
}

#[test]
fn save_metadata_updates_existing_shard_entry_without_dropping_headers() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    let package = make_package();
    let mut updated = make_package();
    updated
        .dist_tags
        .insert("latest".to_string(), "2.0.0".to_string());
    updated.versions.clear();
    updated.versions.insert(
        "2.0.0".to_string(),
        RegistryVersion {
            version: "2.0.0".to_string(),
            dependencies: Default::default(),
            optional_dependencies: Default::default(),
            peer_dependencies: Default::default(),
            peer_dependencies_meta: Default::default(),
            bundled_dependencies: None,
            bundle_dependencies: None,
            dist: RegistryDist {
                tarball: "https://registry.npmjs.org/test-pkg/-/test-pkg-2.0.0.tgz".to_string(),
                integrity: None,
            },
            os: Vec::new(),
            cpu: Vec::new(),
            bin: None,
        },
    );
    let headers = CachedHeaders {
        etag: Some("\"abc123\"".to_string()),
        last_modified: Some("Thu, 01 Jan 2026 00:00:00 GMT".to_string()),
    };

    save_metadata_with_headers(&config, "test-pkg", &package, Some(&headers)).unwrap();
    save_metadata(&config, "test-pkg", &updated).unwrap();

    let loaded = load_metadata(&config, "test-pkg").unwrap();
    assert_eq!(
        loaded.dist_tags.get("latest").map(String::as_str),
        Some("2.0.0")
    );
    assert!(loaded.versions.contains_key("2.0.0"));
    assert!(!loaded.versions.contains_key("1.0.0"));

    let loaded_headers = crate::cache::load_cached_headers(&config, "test-pkg").unwrap();
    assert_eq!(loaded_headers.etag.as_deref(), Some("\"abc123\""));
}

#[test]
fn load_metadata_returns_none_when_not_cached() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());

    let loaded = load_metadata(&config, "nonexistent-pkg");
    assert!(loaded.is_none());
}

#[test]
fn load_metadata_reads_legacy_file_cache() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    let package = make_package();
    let legacy_dir = config.metadata_dir().join("legacy-pkg");

    std::fs::create_dir_all(&legacy_dir).unwrap();
    std::fs::write(
        legacy_dir.join("index.json"),
        serde_json::to_string(&package).unwrap(),
    )
    .unwrap();

    let loaded = load_metadata_with_offline(&config, "legacy-pkg", OfflineMode::PreferOffline);
    assert!(loaded.is_some());
    assert!(loaded.unwrap().versions.contains_key("1.0.0"));
}

#[test]
fn is_fresh_returns_true_for_recent_entry() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    assert!(is_fresh(&config, now));
}

#[test]
fn is_fresh_returns_false_when_no_cache_age_configured() {
    let dir = tempdir().unwrap();
    let mut config = make_config(dir.path().to_path_buf());
    config.min_package_cache_age_days = None;

    assert!(!is_fresh(&config, 0));
}

#[test]
fn is_fresh_returns_false_for_stale_entry() {
    let dir = tempdir().unwrap();
    let config = make_config(dir.path().to_path_buf());
    assert!(!is_fresh(&config, 0));
}
