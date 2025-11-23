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

    let path = if snpm_path.is_file() {
        snpm_path
    } else if pnpm_path.is_file() {
        pnpm_path
    } else {
        return Ok(None);
    };

    let root = dir.to_path_buf();
    let mut cfg = read_config(&path)?;
    merge_snpm_catalog(&root, &mut cfg)?;
    let projects = load_projects(&root, &cfg)?;
    Ok(Workspace {
        root,
        projects,
        config: cfg,
    }
    .into())
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
