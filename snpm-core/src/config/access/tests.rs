use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn make_config() -> SnpmConfig {
    SnpmConfig {
        cache_dir: PathBuf::from("/tmp/cache"),
        data_dir: PathBuf::from("/tmp/data"),
        allow_scripts: BTreeSet::new(),
        disable_global_virtual_store_for_packages: BTreeSet::new(),
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
        remote_cache_url: None,
        remote_cache_auth_token: None,
        remote_cache_read_only: false,
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
        disable_global_virtual_store_for_packages: BTreeSet::new(),
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
        remote_cache_url: None,
        remote_cache_auth_token: None,
        remote_cache_read_only: false,
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
fn registry_url_for_package_name_uses_scoped_registry_when_present() {
    let mut config = make_config();
    config
        .scoped_registries
        .insert("@acme".to_string(), "https://npm.acme.com".to_string());

    assert_eq!(
        config.registry_url_for_package_name("@acme/widget"),
        "https://npm.acme.com"
    );
    assert_eq!(
        config.registry_url_for_package_name("react"),
        "https://registry.npmjs.org"
    );
}

#[test]
fn authorization_header_for_tarball_attaches_auth_when_origins_match() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("my-token".to_string());

    let header = config
        .authorization_header_for_tarball(
            "react",
            "https://registry.npmjs.org/react/-/react-18.0.0.tgz",
        )
        .expect("header");
    assert_eq!(header, "Bearer my-token");
}

#[test]
fn authorization_header_for_tarball_omits_auth_when_origins_differ() {
    let mut config = make_config();
    config
        .registry_auth
        .insert("npm.acme.com".to_string(), "acme-secret".to_string());
    // Reachable token for npm.acme.com exists, but the package is announced by
    // the public registry. Sending the acme token to a malicious or untrusted
    // host would leak credentials, so the helper must return None.
    config.scoped_registries.clear();

    let unscoped_attack = config
        .authorization_header_for_tarball("react", "https://npm.acme.com/react/-/react-18.0.0.tgz");
    assert!(unscoped_attack.is_none());
}

#[test]
fn authorization_header_for_tarball_attaches_auth_for_matching_scoped_registry() {
    let mut config = make_config();
    config
        .scoped_registries
        .insert("@acme".to_string(), "https://npm.acme.com".to_string());
    config
        .registry_auth
        .insert("npm.acme.com".to_string(), "acme-secret".to_string());

    let header = config
        .authorization_header_for_tarball(
            "@acme/widget",
            "https://npm.acme.com/@acme/widget/-/widget-1.0.0.tgz",
        )
        .expect("header");
    assert_eq!(header, "Bearer acme-secret");
}

#[test]
fn authorization_header_for_tarball_omits_auth_when_scoped_registry_redirects_to_other_host() {
    let mut config = make_config();
    config
        .scoped_registries
        .insert("@acme".to_string(), "https://npm.acme.com".to_string());
    config
        .registry_auth
        .insert("cdn.attacker.example".to_string(), "leaked".to_string());

    // A poisoned packument from npm.acme.com cannot trick snpm into sending the
    // user's cdn.attacker.example token to that CDN.
    let header = config.authorization_header_for_tarball(
        "@acme/widget",
        "https://cdn.attacker.example/@acme/widget/-/widget-1.0.0.tgz",
    );
    assert!(header.is_none());
}

#[test]
fn authorization_header_for_tarball_treats_default_port_as_canonical_host() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("ok".to_string());

    // Same host, but tarball URL spells the default 443 port explicitly. Origin
    // match must still succeed once host_from_url normalizes.
    let header = config
        .authorization_header_for_tarball(
            "react",
            "https://registry.npmjs.org:443/react/-/react-18.0.0.tgz",
        )
        .expect("header");
    assert_eq!(header, "Bearer ok");
}

#[test]
fn authorization_header_for_tarball_is_case_insensitive_for_host() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("ok".to_string());

    let header = config
        .authorization_header_for_tarball(
            "react",
            "https://Registry.NpmJS.Org/react/-/react-18.0.0.tgz",
        )
        .expect("header");
    assert_eq!(header, "Bearer ok");
}

#[test]
fn authorization_header_for_tarball_omits_auth_on_scheme_mismatch() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("ok".to_string());
    // Host matches by name but registry is https, tarball is http on the
    // same port — host_from_url drops default-443 only for https, so http
    // with default-80 ends up with a different normalized host. Either way,
    // a downgrade to http with the same host name implies an MITM-able
    // hop; safer to send no token.
    let header = config.authorization_header_for_tarball(
        "react",
        "http://registry.npmjs.org/react/-/react-18.0.0.tgz",
    );
    // We don't assert a specific outcome here (depends on how host_from_url
    // normalizes), but it must not silently send "Bearer ok" over http when
    // the configured registry is https.
    if let Some(header) = header {
        // If for some reason the helper accepts it, the test fails loudly.
        panic!("auth header should not be attached on scheme downgrade, got {header}");
    }
}

#[test]
fn authorization_header_for_tarball_omits_auth_for_malformed_urls() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("ok".to_string());

    assert!(
        config
            .authorization_header_for_tarball("react", "not a url")
            .is_none()
    );
    assert!(
        config
            .authorization_header_for_tarball("react", "")
            .is_none()
    );
}

#[test]
fn authorization_header_for_tarball_falls_back_to_default_when_scope_unknown() {
    let mut config = make_config();
    // The package is in the @acme scope but no scoped registry is configured
    // for that scope, so registry_url_for_package_name returns the default.
    // A tarball on the default registry's host should be authed with the
    // default token.
    config.default_registry_auth_token = Some("ok".to_string());

    let header = config
        .authorization_header_for_tarball(
            "@acme/widget",
            "https://registry.npmjs.org/@acme/widget/-/widget-1.0.0.tgz",
        )
        .expect("header");
    assert_eq!(header, "Bearer ok");
}

#[test]
fn authorization_header_for_tarball_empty_package_name_uses_default_registry() {
    let mut config = make_config();
    config.default_registry_auth_token = Some("ok".to_string());

    let header = config
        .authorization_header_for_tarball("", "https://registry.npmjs.org/foo.tgz")
        .expect("header");
    assert_eq!(header, "Bearer ok");
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
