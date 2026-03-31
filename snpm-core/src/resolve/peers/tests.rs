use super::validate_peers;
use crate::resolve::types::*;

use std::collections::BTreeMap;

fn make_package(name: &str, version: &str) -> (PackageId, ResolvedPackage) {
    let id = PackageId {
        name: name.to_string(),
        version: version.to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: String::new(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
    };
    (id, pkg)
}

#[test]
fn validates_satisfied_peer_dependency() {
    let (react_id, react) = make_package("react", "18.2.0");
    let (plugin_id, mut plugin) = make_package("react-plugin", "1.0.0");
    plugin
        .peer_dependencies
        .insert("react".to_string(), "^18.0.0".to_string());

    let mut packages = BTreeMap::new();
    packages.insert(react_id.clone(), react);
    packages.insert(plugin_id, plugin);

    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages,
    };

    assert!(validate_peers(&graph).is_ok());
}

#[test]
fn rejects_unsatisfied_peer_dependency() {
    let (react_id, react) = make_package("react", "17.0.2");
    let (plugin_id, mut plugin) = make_package("react-plugin", "1.0.0");
    plugin
        .peer_dependencies
        .insert("react".to_string(), "^18.0.0".to_string());

    let mut packages = BTreeMap::new();
    packages.insert(react_id, react);
    packages.insert(plugin_id, plugin);

    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages,
    };

    assert!(validate_peers(&graph).is_err());
}

#[test]
fn rejects_missing_peer_dependency() {
    let (plugin_id, mut plugin) = make_package("react-plugin", "1.0.0");
    plugin
        .peer_dependencies
        .insert("react".to_string(), "^18.0.0".to_string());

    let mut packages = BTreeMap::new();
    packages.insert(plugin_id, plugin);

    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages,
    };

    assert!(validate_peers(&graph).is_err());
}

#[test]
fn no_peer_deps_passes() {
    let (id, pkg) = make_package("lodash", "4.17.21");

    let mut packages = BTreeMap::new();
    packages.insert(id, pkg);

    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages,
    };

    assert!(validate_peers(&graph).is_ok());
}

#[test]
fn empty_graph_passes() {
    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages: BTreeMap::new(),
    };

    assert!(validate_peers(&graph).is_ok());
}
