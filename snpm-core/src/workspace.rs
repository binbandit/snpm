use crate::{Project, Result, SnpmError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct WorkspaceConfig {
    pub packages: Vec<String>,
    #[serde(default)]
    pub catalog: BTreeMap<String, String>,
    #[serde(default)]
    pub catalogs: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default, rename = "onlyBuiltDependencies")]
    pub only_built_dependencies: Vec<String>,
    #[serde(default, rename = "ignoredBuiltDependencies")]
    pub ignored_built_dependencies: Vec<String>,
    #[serde(default)]
    pub hoisting: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogConfig {
    #[serde(default)]
    pub catalog: BTreeMap<String, String>,
    #[serde(default)]
    pub catalogs: BTreeMap<String, BTreeMap<String, String>>,
}

impl CatalogConfig {
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = root.join("snpm-catalog.yaml");
        if !path.is_file() {
            return Ok(None);
        }

        let data = fs::read_to_string(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        let config: Self =
            serde_yaml::from_str(&data).map_err(|err| SnpmError::WorkspaceConfig {
                path: path.clone(),
                reason: err.to_string(),
            })?;

        Ok(Some(config))
    }
}

#[derive(Debug, Deserialize)]
pub struct OverridesConfig {
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
}

impl OverridesConfig {
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let path = root.join("snpm-overrides.yaml");
        if !path.is_file() {
            return Ok(None);
        }

        let data = fs::read_to_string(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        let config: Self =
            serde_yaml::from_str(&data).map_err(|err| SnpmError::WorkspaceConfig {
                path: path.clone(),
                reason: err.to_string(),
            })?;

        Ok(Some(config))
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub root: PathBuf,
    pub projects: Vec<Project>,
    pub config: WorkspaceConfig,
}

impl Workspace {
    pub fn discover(start: &Path) -> Result<Option<Self>> {
        let mut current = Some(start);

        while let Some(dir) = current {
            if let Some(workspace) = try_load_workspace(dir)? {
                return Ok(Some(workspace));
            }

            current = dir.parent();
        }

        Ok(None)
    }

    pub fn project_by_name(&self, name: &str) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| project.manifest.name.as_deref() == Some(name))
    }
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

    if let Some(patterns) = pkg_workspaces {
        for pattern in patterns {
            if !cfg.packages.contains(&pattern) {
                cfg.packages.push(pattern);
            }
        }
    }

    merge_snpm_catalog(&root, &mut cfg)?;
    let projects = load_projects(&root, &cfg)?;
    Ok(Workspace {
        root,
        projects,
        config: cfg,
    }
    .into())
}

fn read_package_json_workspaces(path: &Path) -> Result<Option<Vec<String>>> {
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

    Ok(manifest.workspaces.map(|w| w.patterns().to_vec()))
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

fn merge_snpm_catalog(root: &Path, cfg: &mut WorkspaceConfig) -> Result<()> {
    if let Some(file) = CatalogConfig::load(root)? {
        for (name, range) in file.catalog.iter() {
            cfg.catalog.entry(name.clone()).or_insert(range.clone());
        }

        for (catalog_name, entries) in file.catalogs.iter() {
            let catalog = cfg
                .catalogs
                .entry(catalog_name.clone())
                .or_insert_with(BTreeMap::new);

            for (name, range) in entries.iter() {
                catalog.entry(name.clone()).or_insert(range.clone());
            }
        }
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

    fn temp_dir(name: &str) -> PathBuf {
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

        fs::write(dir.join("snpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{ "name": "my-monorepo", "workspaces": ["apps/*"] }"#,
        )
        .unwrap();
        fs::write(dir.join("packages/a/package.json"), r#"{ "name": "pkg-a" }"#).unwrap();
        fs::write(dir.join("apps/b/package.json"), r#"{ "name": "app-b" }"#).unwrap();

        let workspace = Workspace::discover(&dir).unwrap().unwrap();

        assert!(workspace.config.packages.contains(&"packages/*".to_string()));
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

        fs::write(dir.join("snpm-workspace.yaml"), "packages:\n  - \"packages/*\"\n").unwrap();
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
}
