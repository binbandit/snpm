use super::*;
use crate::registry::{RegistryPackage, RegistryVersion};
use snpm_semver::Version;

#[test]
fn parses_and_matches_simple_range() {
    let ranges = parse_range_set("pkg", ">= 4.21.0").unwrap();
    let v = Version::parse("4.21.0").unwrap();
    assert!(ranges.matches(&v));
}

#[test]
fn selects_custom_dist_tag() {
    let mut versions = std::collections::BTreeMap::new();
    let version_meta = RegistryVersion {
        version: "1.0.0".to_string(),
        dependencies: Default::default(),
        optional_dependencies: Default::default(),
        peer_dependencies: Default::default(),
        peer_dependencies_meta: Default::default(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: crate::registry::RegistryDist {
            tarball: "Url".to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    };
    versions.insert("1.0.0".to_string(), version_meta);

    let mut dist_tags = std::collections::BTreeMap::new();
    dist_tags.insert("ts5.9".to_string(), "1.0.0".to_string());

    let package = RegistryPackage {
        versions,
        time: Default::default(),
        dist_tags,
    };

    let result = select_version("pkg", "ts5.9", &package, None, false);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().version, "1.0.0");
}

fn make_version(ver: &str) -> RegistryVersion {
    RegistryVersion {
        version: ver.to_string(),
        dependencies: Default::default(),
        optional_dependencies: Default::default(),
        peer_dependencies: Default::default(),
        peer_dependencies_meta: Default::default(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: crate::registry::RegistryDist {
            tarball: format!("https://example.com/{}.tgz", ver),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    }
}

fn make_package_with_versions(versions: &[&str]) -> RegistryPackage {
    let mut version_map = std::collections::BTreeMap::new();
    for v in versions {
        version_map.insert(v.to_string(), make_version(v));
    }
    let mut dist_tags = std::collections::BTreeMap::new();
    if let Some(last) = versions.last() {
        dist_tags.insert("latest".to_string(), last.to_string());
    }
    RegistryPackage {
        versions: version_map,
        time: Default::default(),
        dist_tags,
    }
}

#[test]
fn selects_highest_matching_version() {
    let package = make_package_with_versions(&["1.0.0", "1.1.0", "1.2.0", "2.0.0"]);
    let result = select_version("pkg", "^1.0.0", &package, None, false).unwrap();
    assert_eq!(result.version, "1.2.0");
}

#[test]
fn selects_exact_version() {
    let package = make_package_with_versions(&["1.0.0", "1.1.0", "1.2.0"]);
    let result = select_version("pkg", "1.1.0", &package, None, false).unwrap();
    assert_eq!(result.version, "1.1.0");
}

#[test]
fn returns_error_for_no_match() {
    let package = make_package_with_versions(&["1.0.0", "1.1.0"]);
    let result = select_version("pkg", "^2.0.0", &package, None, false);
    assert!(result.is_err());
}

#[test]
fn selects_latest_dist_tag() {
    let package = make_package_with_versions(&["1.0.0", "2.0.0"]);
    let result = select_version("pkg", "latest", &package, None, false).unwrap();
    assert_eq!(result.version, "2.0.0");
}

#[test]
fn parse_range_set_valid() {
    assert!(parse_range_set("pkg", "^1.0.0").is_ok());
}

#[test]
fn parse_range_set_wildcard() {
    let set = parse_range_set("pkg", "*").unwrap();
    let v = Version::parse("999.0.0").unwrap();
    assert!(set.matches(&v));
}

#[test]
fn selects_tilde_range() {
    let package = make_package_with_versions(&["1.0.0", "1.0.5", "1.1.0", "2.0.0"]);
    let result = select_version("pkg", "~1.0.0", &package, None, false).unwrap();
    assert_eq!(result.version, "1.0.5");
}
