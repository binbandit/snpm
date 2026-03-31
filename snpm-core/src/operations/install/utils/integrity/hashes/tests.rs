use super::{NO_PATCH_HASH, compute_lockfile_hash};
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};

use std::collections::BTreeMap;

fn make_graph() -> ResolutionGraph {
    let id = PackageId {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/pkg.tgz".to_string(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
    };
    ResolutionGraph {
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
    }
}

#[test]
fn compute_lockfile_hash_deterministic() {
    let graph = make_graph();
    let hash1 = compute_lockfile_hash(&graph);
    let hash2 = compute_lockfile_hash(&graph);
    assert_eq!(hash1, hash2);
    assert!(!hash1.is_empty());
}

#[test]
fn compute_lockfile_hash_changes_with_different_graph() {
    let graph1 = make_graph();

    let id = PackageId {
        name: "other-pkg".to_string(),
        version: "2.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: "https://example.com/other.tgz".to_string(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
    };
    let graph2 = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::from([(
                "other-pkg".to_string(),
                RootDependency {
                    requested: "^2.0.0".to_string(),
                    resolved: id.clone(),
                },
            )]),
        },
        packages: BTreeMap::from([(id, pkg)]),
    };

    assert_ne!(
        compute_lockfile_hash(&graph1),
        compute_lockfile_hash(&graph2)
    );
}

#[test]
fn no_patch_hash_has_expected_value() {
    assert_eq!(NO_PATCH_HASH, "none");
}
