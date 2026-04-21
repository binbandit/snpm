use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

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
fn authorization_header_uses_scoped_basic_auth() {
    let mut registry_auth = BTreeMap::new();
    registry_auth.insert(
        "registry.example.com".to_string(),
        "dXNlcjpwYXNz".to_string(),
    );

    let mut registry_auth_schemes = BTreeMap::new();
    registry_auth_schemes.insert("registry.example.com".to_string(), AuthScheme::Basic);

    let config = SnpmConfig {
        cache_dir: PathBuf::from("/tmp/cache"),
        data_dir: PathBuf::from("/tmp/data"),
        allow_scripts: BTreeSet::new(),
        min_package_age_days: None,
        min_package_cache_age_days: None,
        default_registry: "https://registry.npmjs.org/".to_string(),
        scoped_registries: BTreeMap::new(),
        registry_auth,
        default_registry_auth_token: None,
        default_registry_auth_scheme: AuthScheme::Bearer,
        registry_auth_schemes,
        hoisting: HoistingMode::SingleVersion,
        link_backend: LinkBackend::Auto,
        strict_peers: false,
        frozen_lockfile_default: false,
        always_auth: false,
        registry_concurrency: 64,
        verbose: false,
        log_file: None,
    };

    let header = config
        .authorization_header_for_url("https://registry.example.com/pkg.tgz")
        .expect("header");

    assert_eq!(header, "Basic dXNlcjpwYXNz");
}

#[test]
fn auth_token_for_url_returns_scoped_token() {
    let mut config = make_config();
    config
        .registry_auth
        .insert("custom.registry.com".to_string(), "my-token".to_string());

    assert_eq!(
        config.auth_token_for_url("https://custom.registry.com/pkg.tgz"),
        Some("my-token")
    );
}

#[test]
fn auth_token_for_url_returns_default_token_for_default_registry() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("default-token".to_string());

    assert_eq!(
        config.auth_token_for_url("https://registry.npmjs.org/pkg.tgz"),
        Some("default-token")
    );
}

#[test]
fn auth_token_for_url_returns_none_for_unknown() {
    let config = make_config();
    assert_eq!(
        config.auth_token_for_url("https://unknown.registry.com/pkg.tgz"),
        None
    );
}

#[test]
fn auth_scheme_for_url_returns_scoped_scheme() {
    let mut config = make_config();
    config
        .registry_auth_schemes
        .insert("custom.registry.com".to_string(), AuthScheme::Basic);

    assert_eq!(
        config.auth_scheme_for_url("https://custom.registry.com/pkg"),
        AuthScheme::Basic
    );
}

#[test]
fn auth_scheme_for_url_returns_default() {
    let config = make_config();
    assert_eq!(
        config.auth_scheme_for_url("https://unknown.com/pkg"),
        AuthScheme::Bearer
    );
}

#[test]
fn authorization_header_bearer() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("my-token".to_string());

    let header = config
        .authorization_header_for_url("https://registry.npmjs.org/pkg")
        .unwrap();
    assert_eq!(header, "Bearer my-token");
}

#[test]
fn authorization_header_returns_none_without_token() {
    let config = make_config();
    assert!(
        config
            .authorization_header_for_url("https://registry.npmjs.org/pkg")
            .is_none()
    );
}

#[test]
fn derived_directories() {
    let config = make_config();
    assert_eq!(
        config.tarball_blob_cache_dir(),
        PathBuf::from("/tmp/cache/tarballs-v1")
    );
    assert_eq!(config.packages_dir(), PathBuf::from("/tmp/data/packages"));
    assert_eq!(
        config.virtual_store_dir(),
        PathBuf::from("/tmp/data/virtual-store")
    );
    assert_eq!(
        config.side_effects_cache_dir(),
        PathBuf::from("/tmp/data/side-effects-v1")
    );
    assert_eq!(config.metadata_dir(), PathBuf::from("/tmp/data/metadata"));
    assert_eq!(
        config.store_residency_index_path(),
        PathBuf::from("/tmp/data/metadata/store-residency-v1.bin")
    );
    assert_eq!(config.global_dir(), PathBuf::from("/tmp/data/global"));
    assert_eq!(config.global_bin_dir(), PathBuf::from("/tmp/data/bin"));
}
