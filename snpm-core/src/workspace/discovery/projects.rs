use super::super::types::WorkspaceConfig;
use crate::{Project, Result, SnpmError};
use std::path::Path;

pub(super) fn load_projects(root: &Path, config: &WorkspaceConfig) -> Result<Vec<Project>> {
    let mut projects = Vec::new();

    for pattern in &config.packages {
        let pattern_path = root.join(pattern);
        let pattern = pattern_path.to_string_lossy().to_string();

        for entry in glob::glob(&pattern).map_err(|error| SnpmError::WorkspaceConfig {
            path: root.to_path_buf(),
            reason: error.to_string(),
        })? {
            let path = entry.map_err(|error| SnpmError::WorkspaceConfig {
                path: root.to_path_buf(),
                reason: error.to_string(),
            })?;

            if path.is_dir() {
                let manifest_path = path.join("package.json");
                if manifest_path.is_file() {
                    projects.push(Project::from_manifest_path(manifest_path)?);
                }
            }
        }
    }

    Ok(projects)
}
