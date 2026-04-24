use super::{populate_virtual_store, rebuild_virtual_store_paths};
use crate::Workspace;
use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};
use crate::workspace::types::WorkspaceConfig;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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

#[test]
fn populate_virtual_store_keeps_workspace_configured_root_packages_local() {
    let dir = tempdir().unwrap();
    let config = SnpmConfig {
        cache_dir: dir.path().join("cache"),
        data_dir: dir.path().join("data"),
        allow_scripts: BTreeSet::new(),
        disable_global_virtual_store_for_packages: BTreeSet::new(),
        min_package_age_days: None,
        min_package_cache_age_days: None,
        default_registry: "https://registry.npmjs.org".to_string(),
        scoped_registries: BTreeMap::new(),
        registry_auth: BTreeMap::new(),
        default_registry_auth_token: None,
        default_registry_auth_scheme: AuthScheme::Bearer,
        registry_auth_schemes: BTreeMap::new(),
        hoisting: HoistingMode::SingleVersion,
        link_backend: LinkBackend::Auto,
        strict_peers: false,
        frozen_lockfile_default: false,
        always_auth: false,
        registry_concurrency: 64,
        verbose: false,
        log_file: None,
    };
    let workspace = Workspace {
        root: dir.path().join("workspace"),
        projects: Vec::new(),
        config: WorkspaceConfig {
            packages: Vec::new(),
            catalog: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            only_built_dependencies: Vec::new(),
            ignored_built_dependencies: Vec::new(),
            disable_global_virtual_store_for_packages: Some(vec!["next".to_string()]),
            hoisting: None,
        },
    };
    let id = PackageId {
        name: "next".to_string(),
        version: "15.0.0".to_string(),
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
            dependencies: BTreeMap::from([(
                id.name.clone(),
                RootDependency {
                    requested: id.version.clone(),
                    resolved: id.clone(),
                },
            )]),
        },
        packages: BTreeMap::from([(id.clone(), pkg)]),
    };
    let store_path = dir.path().join("store/next");
    fs::create_dir_all(&store_path).unwrap();
    fs::write(store_path.join("package.json"), "{}").unwrap();

    let paths = populate_virtual_store(
        &dir.path().join("workspace/.snpm"),
        &graph,
        &BTreeMap::from([(id.clone(), store_path)]),
        &config,
        &workspace,
    )
    .unwrap();
    let local_target = paths.get(&id).unwrap();

    assert!(local_target.is_dir());
    assert!(
        !local_target
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
