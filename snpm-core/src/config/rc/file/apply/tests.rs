use super::apply_rc_file;
use crate::config::rc::types::RegistryConfig;
use crate::config::{AuthScheme, HoistingMode};

use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn parses_scoped_basic_auth_entries() {
    let file = NamedTempFile::new().unwrap();
    fs::write(
        file.path(),
        "//registry.example.com/:_auth=dGVzdDp0b2tlbg==\n",
    )
    .unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert_eq!(
        config
            .registry_auth
            .get("registry.example.com")
            .map(String::as_str),
        Some("dGVzdDp0b2tlbg==")
    );
    assert_eq!(
        config.registry_auth_schemes.get("registry.example.com"),
        Some(&AuthScheme::Basic)
    );
    assert!(config.default_auth_token.is_none());
    assert!(!config.default_auth_basic);
}

#[test]
fn apply_rc_file_parses_registry() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "registry=https://custom.registry.com/\n").unwrap();

    let mut config = RegistryConfig {
        default_registry: "https://registry.npmjs.org/".to_string(),
        ..RegistryConfig::default()
    };
    apply_rc_file(file.path(), &mut config);

    assert_eq!(config.default_registry, "https://custom.registry.com");
}

#[test]
fn apply_rc_file_parses_scoped_registry() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "@myorg:registry=https://npm.myorg.com/\n").unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert_eq!(
        config.scoped.get("@myorg").map(String::as_str),
        Some("https://npm.myorg.com")
    );
}

#[test]
fn apply_rc_file_parses_auth_token() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "//registry.example.com/:_authToken=my-token\n").unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert_eq!(
        config
            .registry_auth
            .get("registry.example.com")
            .map(String::as_str),
        Some("my-token")
    );
    assert_eq!(
        config.registry_auth_schemes.get("registry.example.com"),
        Some(&AuthScheme::Bearer)
    );
}

#[test]
fn apply_rc_file_parses_default_auth_token() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "_authToken=default-token\n").unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert_eq!(config.default_auth_token.as_deref(), Some("default-token"));
}

#[test]
fn apply_rc_file_parses_legacy_auth() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "_auth=dXNlcjpwYXNz\n").unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert_eq!(config.default_auth_token.as_deref(), Some("dXNlcjpwYXNz"));
    assert!(config.default_auth_basic);
}

#[test]
fn apply_rc_file_skips_comments() {
    let file = NamedTempFile::new().unwrap();
    fs::write(
        file.path(),
        "# This is a comment\n; also a comment\nregistry=https://custom.com\n",
    )
    .unwrap();

    let mut config = RegistryConfig {
        default_registry: "https://registry.npmjs.org/".to_string(),
        ..RegistryConfig::default()
    };
    apply_rc_file(file.path(), &mut config);

    assert_eq!(config.default_registry, "https://custom.com");
}

#[test]
fn apply_rc_file_parses_hoisting() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "snpm-hoist=none\n").unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert_eq!(config.hoisting, Some(HoistingMode::None));
}

#[test]
fn apply_rc_file_parses_global_virtual_store_package_list() {
    let file = NamedTempFile::new().unwrap();
    fs::write(
        file.path(),
        r#"disable-global-virtual-store-for-packages=["vite", "next"]"#,
    )
    .unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    let packages = config
        .disable_global_virtual_store_for_packages
        .expect("package list");
    assert!(packages.contains("vite"));
    assert!(packages.contains("next"));
}

#[test]
fn apply_rc_file_parses_always_auth() {
    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "always-auth=true\n").unwrap();

    let mut config = RegistryConfig::default();
    apply_rc_file(file.path(), &mut config);

    assert!(config.always_auth);
}

#[test]
fn apply_rc_file_nonexistent_is_noop() {
    let mut config = RegistryConfig::default();
    apply_rc_file(Path::new("/nonexistent/.snpmrc"), &mut config);
    assert_eq!(config.default_auth_token, None);
}
