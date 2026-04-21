use super::rebuild_virtual_store_paths;
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage};

use std::collections::BTreeMap;
use tempfile::tempdir;

#[test]
fn rebuild_virtual_store_paths_scoped_package() {
    let dir = tempdir().unwrap();
    let store_dir = dir.path().join(".snpm");

    let id = PackageId {
        name: "@scope/pkg".to_string(),
        version: "1.0.0".to_string(),
    };
    let pkg = ResolvedPackage {
        id: id.clone(),
        tarball: String::new(),
        integrity: None,
        dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        bundled_dependencies: None,
        has_bin: false,
        bin: None,
    };
    let graph = ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::new(),
        },
        packages: BTreeMap::from([(id.clone(), pkg)]),
    };

    let paths = rebuild_virtual_store_paths(&store_dir, &graph).unwrap();
    let path = paths.get(&id).unwrap();

    assert!(path.to_string_lossy().contains("@scope+pkg@1.0.0"));
    assert!(path.to_string_lossy().contains("node_modules/@scope/pkg"));
}
