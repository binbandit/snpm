use super::{RegistryDist, RegistryPackage, RegistryVersion};
use crate::registry::BundledDependencies;

use std::collections::BTreeMap;

#[test]
fn registry_version_has_bin_string() {
    let version = RegistryVersion {
        version: "1.0.0".to_string(),
        dependencies: BTreeMap::new(),
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: RegistryDist {
            tarball: "url".to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: Some(serde_json::json!("./cli.js")),
    };
    assert!(version.has_bin());
}

#[test]
fn registry_version_has_bin_null() {
    let version = RegistryVersion {
        version: "1.0.0".to_string(),
        dependencies: BTreeMap::new(),
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: RegistryDist {
            tarball: "url".to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: Some(serde_json::json!(null)),
    };
    assert!(!version.has_bin());
}

#[test]
fn registry_version_has_bin_none() {
    let version = RegistryVersion {
        version: "1.0.0".to_string(),
        dependencies: BTreeMap::new(),
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: RegistryDist {
            tarball: "url".to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    };
    assert!(!version.has_bin());
}

#[test]
fn registry_version_get_bundled_prefers_bundled_dependencies() {
    let version = RegistryVersion {
        version: "1.0.0".to_string(),
        dependencies: BTreeMap::new(),
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: Some(BundledDependencies::List(vec!["a".to_string()])),
        bundle_dependencies: Some(BundledDependencies::List(vec!["b".to_string()])),
        dist: RegistryDist {
            tarball: "url".to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    };
    let bundled = version.get_bundled_dependencies().unwrap();
    match bundled {
        BundledDependencies::List(list) => assert_eq!(list, &vec!["a".to_string()]),
        _ => panic!("expected List"),
    }
}

#[test]
fn registry_version_get_bundled_falls_back_to_bundle() {
    let version = RegistryVersion {
        version: "1.0.0".to_string(),
        dependencies: BTreeMap::new(),
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: None,
        bundle_dependencies: Some(BundledDependencies::List(vec!["b".to_string()])),
        dist: RegistryDist {
            tarball: "url".to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    };
    let bundled = version.get_bundled_dependencies().unwrap();
    match bundled {
        BundledDependencies::List(list) => assert_eq!(list, &vec!["b".to_string()]),
        _ => panic!("expected List"),
    }
}

#[test]
fn registry_package_deserializes_from_json() {
    let json = r#"{
        "versions": {
            "1.0.0": {
                "version": "1.0.0",
                "dist": { "tarball": "https://example.com/pkg.tgz" }
            }
        },
        "dist-tags": { "latest": "1.0.0" }
    }"#;
    let package: RegistryPackage = serde_json::from_str(json).unwrap();
    assert!(package.versions.contains_key("1.0.0"));
    assert_eq!(
        package.dist_tags.get("latest").map(String::as_str),
        Some("1.0.0")
    );
}
