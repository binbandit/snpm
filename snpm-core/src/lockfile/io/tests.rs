use super::{read, write};
use crate::project::BinField;
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
        has_bin: true,
        bin: Some(BinField::Single("cli.js".to_string())),
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
    assert!(matches!(pkg.bin, Some(BinField::Single(ref script)) if script == "cli.js"));
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

#[test]
fn write_emits_binary_sidecar_alongside_yaml() {
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
        bin: None,
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

    assert!(path.is_file(), "yaml lockfile should be written");
    assert!(
        dir.path().join("snpm-lock.bin").is_file(),
        "binary sidecar should be written next to the yaml"
    );
}

#[test]
fn read_falls_back_to_yaml_when_sidecar_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snpm-lock.yaml");

    // Write the canonical lockfile (which also writes the sidecar).
    let id = PackageId {
        name: "kept".to_string(),
        version: "1.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/kept-1.0.0.tgz".to_string(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
        bin: None,
    };
    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::from([(
                "kept".to_string(),
                RootDependency {
                    requested: "^1.0.0".to_string(),
                    resolved: id.clone(),
                },
            )]),
        },
        packages: BTreeMap::from([(id, pkg)]),
    };
    write(&path, &graph, &BTreeMap::new()).unwrap();

    // Hand-edit the YAML so its hash no longer matches the sidecar. The
    // sidecar's bincode payload still describes the original tarball; the
    // YAML now describes a different one. After invalidation, read() should
    // surface the YAML's view.
    let yaml = std::fs::read_to_string(&path).unwrap();
    let edited_yaml = yaml.replace(
        "https://example.com/kept-1.0.0.tgz",
        "https://example.com/edited-1.0.0.tgz",
    );
    std::fs::write(&path, &edited_yaml).unwrap();

    let lockfile = read(&path).unwrap();
    let entry = lockfile
        .packages
        .get("kept@1.0.0")
        .expect("kept@1.0.0 entry");
    assert_eq!(entry.tarball, "https://example.com/edited-1.0.0.tgz");
}

#[test]
fn read_falls_back_to_yaml_when_sidecar_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snpm-lock.yaml");
    let data = "version: 1\nroot:\n  dependencies: {}\npackages: {}\n";
    std::fs::write(&path, data).unwrap();

    let lockfile = read(&path).unwrap();
    assert_eq!(lockfile.version, 1);
    assert!(lockfile.packages.is_empty());
}
