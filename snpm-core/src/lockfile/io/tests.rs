use super::{read, write};
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};

use std::collections::BTreeMap;

#[test]
fn write_and_read_round_trip() {
    let id = PackageId {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/test-pkg-1.0.0.tgz".to_string(),
        integrity: Some("sha512-abc".to_string()),
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
    };

    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::from([(
                "test-pkg".to_string(),
                RootDependency {
                    requested: "^1.0.0".to_string(),
                    resolved: id.clone(),
                },
            )]),
        },
        packages: BTreeMap::from([(id, pkg)]),
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snpm-lock.yaml");

    write(&path, &graph, &BTreeMap::new()).unwrap();
    let lockfile = read(&path).unwrap();

    assert_eq!(lockfile.version, 1);
    assert!(lockfile.root.dependencies.contains_key("test-pkg"));
    assert_eq!(lockfile.root.dependencies["test-pkg"].requested, "^1.0.0");
    assert!(lockfile.packages.contains_key("test-pkg@1.0.0"));
    let pkg = &lockfile.packages["test-pkg@1.0.0"];
    assert_eq!(pkg.tarball, "https://example.com/test-pkg-1.0.0.tgz");
    assert_eq!(pkg.integrity.as_deref(), Some("sha512-abc"));
}

#[test]
fn read_rejects_wrong_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snpm-lock.yaml");

    let data = "version: 99\nroot:\n  dependencies: {}\npackages: {}\n";
    std::fs::write(&path, data).unwrap();

    let result = read(&path);
    assert!(result.is_err());
}
