use super::types::{CatalogConfig, Workspace, WorkspaceConfig};
use crate::project::{CatalogMap, NamedCatalogsMap};
use crate::{Project, Result, SnpmError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn discover_workspace(start: &Path) -> Result<Option<Workspace>> {
    let mut current = Some(start);

    while let Some(dir) = current {
        if let Some(workspace) = try_load_workspace(dir)? {
            return Ok(Some(workspace));
        }

        current = dir.parent();
    }

    Ok(None)
}

fn try_load_workspace(dir: &Path) -> Result<Option<Workspace>> {
    let snpm_path = dir.join("snpm-workspace.yaml");
    let pnpm_path = dir.join("pnpm-workspace.yaml");
    let package_json_path = dir.join("package.json");

    let yaml_path = if snpm_path.is_file() {
        Some(snpm_path)
    } else if pnpm_path.is_file() {
        Some(pnpm_path)
    } else {
        None
    };

    let pkg_workspaces = if package_json_path.is_file() {
        read_package_json_workspaces(&package_json_path)?
    } else {
        None
    };

    if yaml_path.is_none() && pkg_workspaces.is_none() {
        return Ok(None);
    }

    let root = dir.to_path_buf();

    let mut cfg = if let Some(path) = yaml_path {
        read_config(&path)?
    } else {
        WorkspaceConfig {
            packages: Vec::new(),
            catalog: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            only_built_dependencies: Vec::new(),
            ignored_built_dependencies: Vec::new(),
            hoisting: None,
        }
    };

    if let Some((patterns, catalog, catalogs)) = pkg_workspaces.map(|w| w.into_parts()) {
        for pattern in patterns {
            if !cfg.packages.contains(&pattern) {
                cfg.packages.push(pattern);
            }
        }
        merge_catalog_entries(&mut cfg, catalog, catalogs);
    }

    merge_snpm_catalog(&root, &mut cfg)?;
    let projects = load_projects(&root, &cfg)?;
    Ok(Some(Workspace {
        root,
        projects,
        config: cfg,
    }))
}

fn read_package_json_workspaces(path: &Path) -> Result<Option<crate::project::WorkspacesField>> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    #[derive(Deserialize)]
    struct PartialManifest {
        workspaces: Option<crate::project::WorkspacesField>,
    }

    let manifest: PartialManifest =
        serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
            path: path.to_path_buf(),
            source,
        })?;

    Ok(manifest.workspaces)
}

fn read_config(path: &Path) -> Result<WorkspaceConfig> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let cfg: WorkspaceConfig =
        serde_yaml::from_str(&data).map_err(|err| SnpmError::WorkspaceConfig {
            path: path.to_path_buf(),
            reason: err.to_string(),
        })?;

    Ok(cfg)
}

/// Merge catalog and named catalogs into `cfg`, keeping existing entries (higher priority).
fn merge_catalog_entries(
    cfg: &mut WorkspaceConfig,
    catalog: CatalogMap,
    catalogs: NamedCatalogsMap,
) {
    for (name, range) in catalog {
        cfg.catalog.entry(name).or_insert(range);
    }
    for (catalog_name, entries) in catalogs {
        let target = cfg.catalogs.entry(catalog_name).or_default();
        for (name, range) in entries {
            target.entry(name).or_insert(range);
        }
    }
}

fn merge_snpm_catalog(root: &Path, cfg: &mut WorkspaceConfig) -> Result<()> {
    if let Some(file) = CatalogConfig::load(root)? {
        merge_catalog_entries(cfg, file.catalog, file.catalogs);
    }
    Ok(())
}

fn load_projects(root: &Path, cfg: &WorkspaceConfig) -> Result<Vec<Project>> {
    let mut projects = Vec::new();

    for pattern in cfg.packages.iter() {
        let pattern_path = root.join(pattern);
        let pattern_str = pattern_path.to_string_lossy().to_string();

        for entry in glob::glob(&pattern_str).map_err(|err| SnpmError::WorkspaceConfig {
            path: root.to_path_buf(),
            reason: err.to_string(),
        })? {
            let path = entry.map_err(|err| SnpmError::WorkspaceConfig {
                path: root.to_path_buf(),
                reason: err.to_string(),
            })?;

            if path.is_dir() {
                let manifest_path = path.join("package.json");
                if manifest_path.is_file() {
                    let project = Project::from_manifest_path(manifest_path)?;
                    projects.push(project);
                }
            }
        }
    }

    Ok(projects)
}

#[cfg(test)]
mod tests {
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
        assert_eq!(workspace.projects.len(), 1);
        assert_eq!(workspace.projects[0].manifest.name.as_deref(), Some("foo"));

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
        assert_eq!(workspace.projects.len(), 1);
        assert_eq!(workspace.projects[0].manifest.name.as_deref(), Some("bar"));

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
        assert_eq!(workspace.projects.len(), 2);

        let names: Vec<_> = workspace
            .projects
            .iter()
            .filter_map(|p| p.manifest.name.as_deref())
            .collect();
        assert!(names.contains(&"pkg-a"));
        assert!(names.contains(&"app-b"));

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
        assert_eq!(workspace.projects.len(), 1);

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

        // YAML workspace catalog wins over package.json catalog
        assert_eq!(
            workspace.config.catalog.get("react").map(|s| s.as_str()),
            Some("18.0.0")
        );
        // package.json catalog entry is added when not in YAML
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
}
