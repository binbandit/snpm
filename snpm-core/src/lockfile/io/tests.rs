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

#[test]
fn read_falls_back_to_yaml_when_sidecar_has_bad_magic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snpm-lock.yaml");
    let sidecar = dir.path().join("snpm-lock.bin");
    let data = "version: 1\nroot:\n  dependencies: {}\npackages: {}\n";
    std::fs::write(&path, data).unwrap();
    std::fs::write(&sidecar, vec![0_u8; 200]).unwrap();

    // Garbage sidecar (zero header, no magic) — the read must fall back to
    // YAML rather than surface an error.
    let lockfile = read(&path).unwrap();
    assert_eq!(lockfile.version, 1);
}

#[test]
fn read_falls_back_to_yaml_when_sidecar_is_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snpm-lock.yaml");
    let sidecar = dir.path().join("snpm-lock.bin");
    let data = "version: 1\nroot:\n  dependencies: {}\npackages: {}\n";
    std::fs::write(&path, data).unwrap();
    // 4 bytes — shorter than even the header.
    std::fs::write(&sidecar, b"SNPB").unwrap();

    let lockfile = read(&path).unwrap();
    assert_eq!(lockfile.version, 1);
}

#[test]
fn write_then_read_round_trips_repeatedly_via_sidecar() {
    let id = PackageId {
        name: "round-trip".to_string(),
        version: "1.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/round-trip-1.0.0.tgz".to_string(),
        integrity: Some("sha512-zzz".to_string()),
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
        bin: None,
    };
    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::from([(
                "round-trip".to_string(),
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

    for _ in 0..5 {
        write(&path, &graph, &BTreeMap::new()).unwrap();
        let lockfile = read(&path).unwrap();
        assert_eq!(lockfile.packages.len(), 1);
        let entry = &lockfile.packages["round-trip@1.0.0"];
        assert_eq!(entry.tarball, "https://example.com/round-trip-1.0.0.tgz");
    }
}

#[test]
fn whitespace_only_yaml_edit_invalidates_sidecar() {
    // The sidecar's embedded SHA-256 is over raw YAML bytes — even a
    // semantically inert whitespace change must invalidate it so the
    // sidecar can't lie about what the YAML currently says.
    let id = PackageId {
        name: "pkg".to_string(),
        version: "1.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/pkg-1.0.0.tgz".to_string(),
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
                "pkg".to_string(),
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

    // Append a trailing newline (whitespace-only). The YAML still parses to
    // the same Lockfile, but the bytes differ — sidecar must be rejected.
    let mut yaml = std::fs::read_to_string(&path).unwrap();
    yaml.push('\n');
    std::fs::write(&path, &yaml).unwrap();

    let lockfile = read(&path).unwrap();
    assert_eq!(lockfile.packages.len(), 1);
    // Round-tripping again should resync the sidecar.
    write(&path, &lockfile_to_graph(&lockfile), &BTreeMap::new()).unwrap();
    let again = read(&path).unwrap();
    assert_eq!(again.packages.len(), 1);
}

/// Test helper: turn a Lockfile back into a minimal ResolutionGraph so we
/// can use the write() entrypoint with read-back data.
fn lockfile_to_graph(lockfile: &crate::lockfile::types::Lockfile) -> ResolutionGraph {
    use crate::resolve::{PackageId, ResolvedPackage};

    let root_deps = lockfile
        .root
        .dependencies
        .iter()
        .filter_map(|(name, entry)| {
            entry.version.as_ref().map(|version| {
                (
                    name.clone(),
                    RootDependency {
                        requested: entry.requested.clone(),
                        resolved: PackageId {
                            name: entry
                                .package
                                .clone()
                                .unwrap_or_else(|| name.clone()),
                            version: version.clone(),
                        },
                    },
                )
            })
        })
        .collect();
    let packages = lockfile
        .packages
        .values()
        .map(|entry| {
            let id = PackageId {
                name: entry.name.clone(),
                version: entry.version.clone(),
            };
            let pkg = ResolvedPackage {
                id: id.clone(),
                tarball: entry.tarball.clone(),
                integrity: entry.integrity.clone(),
                dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: entry.bundled_dependencies.clone(),
                has_bin: entry.has_bin,
                bin: entry.bin.clone(),
            };
            (id, pkg)
        })
        .collect();
    ResolutionGraph {
        root: ResolutionRoot {
            dependencies: root_deps,
        },
        packages,
    }
}
