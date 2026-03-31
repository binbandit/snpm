use super::*;
use tempfile::tempdir;

#[test]
fn manifest_deserializes_minimal() {
    let json = r#"{ "name": "my-pkg", "version": "1.0.0" }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    assert_eq!(manifest.name.as_deref(), Some("my-pkg"));
    assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
    assert!(manifest.dependencies.is_empty());
    assert!(manifest.dev_dependencies.is_empty());
}

#[test]
fn manifest_deserializes_with_deps() {
    let json = r#"{
        "name": "test",
        "dependencies": { "lodash": "^4.0.0" },
        "devDependencies": { "jest": "^29.0.0" }
    }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    assert_eq!(
        manifest.dependencies.get("lodash").map(String::as_str),
        Some("^4.0.0")
    );
    assert_eq!(
        manifest.dev_dependencies.get("jest").map(String::as_str),
        Some("^29.0.0")
    );
}

#[test]
fn manifest_deserializes_scripts() {
    let json = r#"{ "scripts": { "build": "tsc", "test": "jest" } }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    assert_eq!(
        manifest.scripts.get("build").map(String::as_str),
        Some("tsc")
    );
}

#[test]
fn manifest_deserializes_bin_single() {
    let json = r#"{ "bin": "./cli.js" }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    assert!(matches!(manifest.bin, Some(BinField::Single(_))));
}

#[test]
fn manifest_deserializes_bin_map() {
    let json = r#"{ "bin": { "cmd1": "./a.js", "cmd2": "./b.js" } }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    assert!(matches!(manifest.bin, Some(BinField::Map(_))));
}

#[test]
fn workspaces_field_patterns_array() {
    let json = r#"{ "workspaces": ["packages/*"] }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    let workspaces = manifest.workspaces.unwrap();
    assert_eq!(workspaces.patterns(), &["packages/*".to_string()]);
}

#[test]
fn workspaces_field_object() {
    let json = r#"{ "workspaces": { "packages": ["apps/*"], "catalog": { "react": "^18.0.0" } } }"#;
    let manifest: Manifest = serde_json::from_str(json).unwrap();
    let workspaces = manifest.workspaces.unwrap();
    assert_eq!(workspaces.patterns(), &["apps/*".to_string()]);
    let (patterns, catalog, _catalogs) = workspaces.into_parts();
    assert_eq!(patterns, vec!["apps/*".to_string()]);
    assert_eq!(catalog.get("react").map(String::as_str), Some("^18.0.0"));
}

#[test]
fn workspaces_into_parts_patterns() {
    let workspaces = WorkspacesField::Patterns(vec!["a/*".to_string(), "b/*".to_string()]);
    let (patterns, catalog, catalogs) = workspaces.into_parts();
    assert_eq!(patterns, vec!["a/*", "b/*"]);
    assert!(catalog.is_empty());
    assert!(catalogs.is_empty());
}

#[test]
fn project_discover_finds_manifest() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("packages/foo");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(dir.path().join("package.json"), r#"{ "name": "root" }"#).unwrap();

    let project = Project::discover(&sub).unwrap();
    assert_eq!(project.manifest.name.as_deref(), Some("root"));
}

#[test]
fn project_discover_fails_without_manifest() {
    let dir = tempdir().unwrap();
    let result = Project::discover(dir.path());
    assert!(result.is_err());
}

#[test]
fn project_from_manifest_path() {
    let dir = tempdir().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(
        &manifest_path,
        r#"{ "name": "test-pkg", "version": "2.0.0" }"#,
    )
    .unwrap();

    let project = Project::from_manifest_path(manifest_path).unwrap();
    assert_eq!(project.manifest.name.as_deref(), Some("test-pkg"));
    assert_eq!(project.manifest.version.as_deref(), Some("2.0.0"));
    assert_eq!(project.root, dir.path());
}

#[test]
fn project_write_manifest_roundtrip() {
    let dir = tempdir().unwrap();
    let manifest_path = dir.path().join("package.json");
    std::fs::write(&manifest_path, r#"{ "name": "original" }"#).unwrap();

    let project = Project::from_manifest_path(manifest_path).unwrap();

    let mut modified = project.manifest.clone();
    modified.name = Some("modified".to_string());
    project.write_manifest(&modified).unwrap();

    let reloaded = Project::from_manifest_path(project.manifest_path).unwrap();
    assert_eq!(reloaded.manifest.name.as_deref(), Some("modified"));
}
