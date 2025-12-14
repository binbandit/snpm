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
        crate::workspace::discovery::discover_workspace(start)
    }

    pub fn project_by_name(&self, name: &str) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| project.manifest.name.as_deref() == Some(name))
    }
}
