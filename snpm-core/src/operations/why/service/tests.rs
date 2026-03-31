use super::paths::{build_match, walk_paths};
use crate::operations::why::index::build_reverse_index;
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};

use std::collections::{BTreeMap, BTreeSet};

fn package_id(name: &str, version: &str) -> PackageId {
    PackageId {
        name: name.to_string(),
        version: version.to_string(),
    }
}

fn graph_fixture() -> ResolutionGraph {
    let target = package_id("target", "1.0.0");
    let mid = package_id("mid", "1.0.0");
    let top = package_id("top", "1.0.0");

    let mut packages = BTreeMap::new();

    packages.insert(
        target.clone(),
        ResolvedPackage {
            id: target.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
        },
    );

    packages.insert(
        mid.clone(),
        ResolvedPackage {
            id: mid.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::from([("target".to_string(), target.clone())]),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
        },
    );

    packages.insert(
        top.clone(),
        ResolvedPackage {
            id: top.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::from([("mid".to_string(), mid.clone())]),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
        },
    );

    ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::from([(
                "top".to_string(),
                RootDependency {
                    requested: "^1.0.0".to_string(),
                    resolved: top,
                },
            )]),
        },
        packages,
    }
}

#[test]
fn collects_reverse_path_to_root() {
    let graph = graph_fixture();
    let index = build_reverse_index(&graph);
    let target = package_id("target", "1.0.0");

    let result = build_match(&target, &index, usize::MAX);

    assert_eq!(result.paths.len(), 1);
    assert!(!result.paths[0].truncated);
    assert_eq!(result.paths[0].hops.len(), 3);
}

#[test]
fn truncates_when_depth_reached() {
    let graph = graph_fixture();
    let index = build_reverse_index(&graph);
    let target = package_id("target", "1.0.0");
    let mut visited = BTreeSet::new();
    visited.insert(target.clone());

    let mut paths = Vec::new();
    walk_paths(&target, &index, 1, &mut visited, Vec::new(), 0, &mut paths);

    assert_eq!(paths.len(), 1);
    assert!(paths[0].truncated);
    assert_eq!(paths[0].hops.len(), 1);
}
