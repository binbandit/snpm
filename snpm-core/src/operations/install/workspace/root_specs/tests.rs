use super::*;
use crate::Workspace;
use crate::project::Manifest;
use std::collections::BTreeMap;

fn make_workspace_with_project(name: &str, version: Option<&str>) -> Workspace {
    let dir = std::env::temp_dir().join(format!("snpm_test_ws_{}", std::process::id()));
    let project = crate::Project {
        root: dir.join(name),
        manifest_path: dir.join(name).join("package.json"),
        manifest: Manifest {
            name: Some(name.to_string()),
            version: version.map(str::to_string),
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

    Workspace {
        root: dir,
        projects: vec![project],
        config: crate::workspace::types::WorkspaceConfig {
            packages: Vec::new(),
            catalog: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            only_built_dependencies: Vec::new(),
            ignored_built_dependencies: Vec::new(),
            hoisting: None,
        },
    }
}

#[test]
fn validate_workspace_spec_wildcard() {
    let ws = make_workspace_with_project("my-lib", Some("1.0.0"));
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:*").is_ok());
}

#[test]
fn validate_workspace_spec_empty() {
    let ws = make_workspace_with_project("my-lib", Some("1.0.0"));
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:").is_ok());
}

#[test]
fn validate_workspace_spec_caret() {
    let ws = make_workspace_with_project("my-lib", Some("1.2.3"));
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^").is_ok());
}

#[test]
fn validate_workspace_spec_tilde() {
    let ws = make_workspace_with_project("my-lib", Some("1.2.3"));
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:~").is_ok());
}

#[test]
fn validate_workspace_spec_explicit_range_satisfied() {
    let ws = make_workspace_with_project("my-lib", Some("1.2.3"));
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^1.0.0").is_ok());
}

#[test]
fn validate_workspace_spec_explicit_range_not_satisfied() {
    let ws = make_workspace_with_project("my-lib", Some("1.2.3"));
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^2.0.0").is_err());
}

#[test]
fn validate_workspace_spec_missing_project() {
    let ws = make_workspace_with_project("my-lib", Some("1.0.0"));
    assert!(validate_workspace_spec(&ws, "nonexistent", "workspace:*").is_err());
}

#[test]
fn validate_workspace_spec_allows_versionless_local_projects() {
    let ws = make_workspace_with_project("my-lib", None);
    assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^").is_ok());
}

#[test]
fn insert_workspace_root_dep_prefers_more_specific_compatible_range() {
    let workspace = make_workspace_with_project("app", Some("1.0.0"));
    let mut combined = BTreeMap::new();

    insert_workspace_root_dep(
        &mut combined,
        &workspace.root,
        &workspace.root.join("a"),
        "typescript",
        "^5.0.0",
    )
    .unwrap();
    insert_workspace_root_dep(
        &mut combined,
        &workspace.root,
        &workspace.root.join("b"),
        "typescript",
        "^5.1.3",
    )
    .unwrap();

    assert_eq!(
        combined.get("typescript").map(String::as_str),
        Some("^5.1.3")
    );
}
