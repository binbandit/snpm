use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("snpm_test_{}_{}", name, timestamp))
}

#[test]
fn test_catalog_overlay() {
    let dir = temp_dir("catalog_overlay");
    fs::create_dir_all(&dir).unwrap();

    let workspace_yaml = r#"
packages: []
catalog:
  react: "18.0.0"
  lodash: "4.0.0"
"#;
    fs::write(dir.join("snpm-workspace.yaml"), workspace_yaml).unwrap();

    let catalog_yaml = r#"
catalog:
  react: "17.0.0" # Should be ignored (workspace takes precedence)
  axios: "1.0.0"  # Should be added
"#;
    fs::write(dir.join("snpm-catalog.yaml"), catalog_yaml).unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert_eq!(
        workspace.config.catalog.get("react").map(|s| s.as_str()),
        Some("18.0.0")
    );
    assert_eq!(
        workspace.config.catalog.get("lodash").map(|s| s.as_str()),
        Some("4.0.0")
    );
    assert_eq!(
        workspace.config.catalog.get("axios").map(|s| s.as_str()),
        Some("1.0.0")
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_package_json_workspaces_array() {
    let dir = temp_dir("pkg_json_workspaces_array");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(dir.join("packages/foo")).unwrap();

    fs::write(
        dir.join("package.json"),
        r#"{ "name": "my-monorepo", "workspaces": ["packages/*"] }"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/foo/package.json"),
        r#"{ "name": "foo", "version": "1.0.0" }"#,
    )
    .unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert_eq!(workspace.config.packages, vec!["packages/*"]);
    assert_eq!(workspace.projects.len(), 2);

    let names: Vec<_> = workspace
        .projects
        .iter()
        .filter_map(|project| project.manifest.name.as_deref())
        .collect();
    assert!(names.contains(&"my-monorepo"));
    assert!(names.contains(&"foo"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_package_json_workspaces_object() {
    let dir = temp_dir("pkg_json_workspaces_object");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(dir.join("apps/bar")).unwrap();

    fs::write(
        dir.join("package.json"),
        r#"{ "name": "my-monorepo", "workspaces": { "packages": ["apps/*"] } }"#,
    )
    .unwrap();
    fs::write(
        dir.join("apps/bar/package.json"),
        r#"{ "name": "bar", "version": "2.0.0" }"#,
    )
    .unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert_eq!(workspace.config.packages, vec!["apps/*"]);
    assert_eq!(workspace.projects.len(), 2);

    let names: Vec<_> = workspace
        .projects
        .iter()
        .filter_map(|project| project.manifest.name.as_deref())
        .collect();
    assert!(names.contains(&"my-monorepo"));
    assert!(names.contains(&"bar"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_combined_yaml_and_package_json_workspaces() {
    let dir = temp_dir("combined_workspaces");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(dir.join("packages/a")).unwrap();
    fs::create_dir_all(dir.join("apps/b")).unwrap();

    fs::write(
        dir.join("snpm-workspace.yaml"),
        "packages:\n  - \"packages/*\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("package.json"),
        r#"{ "name": "my-monorepo", "workspaces": ["apps/*"] }"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/a/package.json"),
        r#"{ "name": "pkg-a" }"#,
    )
    .unwrap();
    fs::write(dir.join("apps/b/package.json"), r#"{ "name": "app-b" }"#).unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert!(
        workspace
            .config
            .packages
            .contains(&"packages/*".to_string())
    );
    assert!(workspace.config.packages.contains(&"apps/*".to_string()));
    assert_eq!(workspace.projects.len(), 3);

    let names: Vec<_> = workspace
        .projects
        .iter()
        .filter_map(|project| project.manifest.name.as_deref())
        .collect();
    assert!(names.contains(&"pkg-a"));
    assert!(names.contains(&"app-b"));
    assert!(names.contains(&"my-monorepo"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_no_duplicate_patterns() {
    let dir = temp_dir("no_duplicate_patterns");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("snpm-workspace.yaml"),
        "packages:\n  - \"packages/*\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("package.json"),
        r#"{ "name": "my-monorepo", "workspaces": ["packages/*"] }"#,
    )
    .unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert_eq!(workspace.config.packages.len(), 1);
    assert_eq!(workspace.config.packages[0], "packages/*");

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_package_json_workspaces_object_with_catalog() {
    let dir = temp_dir("pkg_json_catalog");
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(dir.join("packages/foo")).unwrap();

    fs::write(
        dir.join("package.json"),
        r#"{
            "name": "my-monorepo",
            "workspaces": {
                "packages": ["packages/*"],
                "catalog": {
                    "typescript": "^5.7.3",
                    "react": "^18.2.0"
                }
            }
        }"#,
    )
    .unwrap();
    fs::write(
        dir.join("packages/foo/package.json"),
        r#"{ "name": "foo", "version": "1.0.0" }"#,
    )
    .unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert_eq!(workspace.config.packages, vec!["packages/*"]);
    assert_eq!(
        workspace
            .config
            .catalog
            .get("typescript")
            .map(|s| s.as_str()),
        Some("^5.7.3")
    );
    assert_eq!(
        workspace.config.catalog.get("react").map(|s| s.as_str()),
        Some("^18.2.0")
    );
    assert_eq!(workspace.projects.len(), 2);

    let names: Vec<_> = workspace
        .projects
        .iter()
        .filter_map(|project| project.manifest.name.as_deref())
        .collect();
    assert!(names.contains(&"my-monorepo"));
    assert!(names.contains(&"foo"));

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_yaml_catalog_takes_priority_over_package_json_catalog() {
    let dir = temp_dir("yaml_catalog_priority");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("snpm-workspace.yaml"),
        "packages: []\ncatalog:\n  react: \"18.0.0\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("package.json"),
        r#"{
            "name": "my-monorepo",
            "workspaces": {
                "packages": [],
                "catalog": {
                    "react": "^17.0.0",
                    "typescript": "^5.7.3"
                }
            }
        }"#,
    )
    .unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    assert_eq!(
        workspace.config.catalog.get("react").map(|s| s.as_str()),
        Some("18.0.0")
    );
    assert_eq!(
        workspace
            .config
            .catalog
            .get("typescript")
            .map(|s| s.as_str()),
        Some("^5.7.3")
    );

    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn test_package_json_workspaces_object_with_named_catalogs() {
    let dir = temp_dir("pkg_json_named_catalogs");
    fs::create_dir_all(&dir).unwrap();

    fs::write(
        dir.join("package.json"),
        r#"{
            "name": "my-monorepo",
            "workspaces": {
                "packages": [],
                "catalogs": {
                    "build": {
                        "vite": "^5.0.0",
                        "esbuild": "^0.19.0"
                    }
                }
            }
        }"#,
    )
    .unwrap();

    let workspace = Workspace::discover(&dir).unwrap().unwrap();

    let build = workspace.config.catalogs.get("build").unwrap();
    assert_eq!(build.get("vite").map(|s| s.as_str()), Some("^5.0.0"));
    assert_eq!(build.get("esbuild").map(|s| s.as_str()), Some("^0.19.0"));

    fs::remove_dir_all(&dir).unwrap();
}
