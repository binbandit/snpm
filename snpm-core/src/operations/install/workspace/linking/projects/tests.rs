use super::workspace::collect_workspace_protocol_deps;
use crate::Project;
use crate::project::Manifest;
use crate::workspace::types::{Workspace, WorkspaceConfig};

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn collect_workspace_protocol_deps_filters_correctly() {
    let workspace_project = Project {
        root: PathBuf::from("/tmp/lib-a"),
        manifest_path: PathBuf::from("/tmp/lib-a/package.json"),
        manifest: Manifest {
            name: Some("lib-a".to_string()),
            version: Some("1.2.3".to_string()),
            private: false,
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            scripts: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            files: None,
            bin: None,
            main: None,
            pnpm: None,
            snpm: None,
            workspaces: None,
        },
    };
    let workspace = Workspace {
        root: PathBuf::from("/tmp"),
        projects: vec![workspace_project],
        config: WorkspaceConfig {
            packages: Vec::new(),
            catalog: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            only_built_dependencies: Vec::new(),
            ignored_built_dependencies: Vec::new(),
            hoisting: None,
        },
    };
    let project = Project {
        root: PathBuf::from("/tmp/project"),
        manifest_path: PathBuf::from("/tmp/project/package.json"),
        manifest: Manifest {
            name: Some("test".to_string()),
            version: None,
            private: false,
            dependencies: BTreeMap::from([
                ("lib-a".to_string(), "workspace:*".to_string()),
                ("lodash".to_string(), "^4.0.0".to_string()),
            ]),
            dev_dependencies: BTreeMap::from([
                ("lib-b".to_string(), "workspace:^".to_string()),
                ("jest".to_string(), "^29.0.0".to_string()),
            ]),
            optional_dependencies: BTreeMap::from([(
                "lib-c".to_string(),
                "workspace:~".to_string(),
            )]),
            scripts: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            files: None,
            bin: None,
            main: None,
            pnpm: None,
            snpm: None,
            workspaces: None,
        },
    };

    let (deps, dev_deps, optional_deps) =
        collect_workspace_protocol_deps(&project, &workspace).unwrap();
    assert!(deps.contains("lib-a"));
    assert!(!deps.contains("lodash"));
    assert!(!dev_deps.contains("lib-b"));
    assert!(!dev_deps.contains("jest"));
    assert!(!optional_deps.contains("lib-c"));
}

#[test]
fn collect_workspace_protocol_deps_includes_semver_matched_local_packages() {
    let local_project = Project {
        root: PathBuf::from("/tmp/shared"),
        manifest_path: PathBuf::from("/tmp/shared/package.json"),
        manifest: Manifest {
            name: Some("shared".to_string()),
            version: Some("1.0.0".to_string()),
            private: false,
            dependencies: BTreeMap::new(),
            dev_dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            scripts: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            files: None,
            bin: None,
            main: None,
            pnpm: None,
            snpm: None,
            workspaces: None,
        },
    };
    let workspace = Workspace {
        root: PathBuf::from("/tmp"),
        projects: vec![local_project],
        config: WorkspaceConfig {
            packages: Vec::new(),
            catalog: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            only_built_dependencies: Vec::new(),
            ignored_built_dependencies: Vec::new(),
            hoisting: None,
        },
    };
    let project = Project {
        root: PathBuf::from("/tmp/app"),
        manifest_path: PathBuf::from("/tmp/app/package.json"),
        manifest: Manifest {
            name: Some("app".to_string()),
            version: Some("1.0.0".to_string()),
            private: false,
            dependencies: BTreeMap::from([("shared".to_string(), "^1.0.0".to_string())]),
            dev_dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            scripts: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            files: None,
            bin: None,
            main: None,
            pnpm: None,
            snpm: None,
            workspaces: None,
        },
    };

    let (deps, _, _) = collect_workspace_protocol_deps(&project, &workspace).unwrap();
    assert!(deps.contains("shared"));
}
