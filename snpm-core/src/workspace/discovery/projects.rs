use super::super::types::WorkspaceConfig;
use crate::{Project, Result, SnpmError};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) fn load_projects(
    root: &Path,
    config: &WorkspaceConfig,
    include_root_project: bool,
) -> Result<Vec<Project>> {
    let mut projects = Vec::new();
    let mut seen_manifests = BTreeSet::new();
    let exclusions = build_exclusion_patterns(root, &config.packages)?;

    let root_manifest = root.join("package.json");
    if include_root_project && root_manifest.is_file() {
        push_project(&root_manifest, &mut projects, &mut seen_manifests)?;
    }

    for pattern in config
        .packages
        .iter()
        .filter(|pattern| !pattern.starts_with('!'))
    {
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

            if path.is_dir() && !is_excluded(&path, &exclusions) {
                let manifest_path = path.join("package.json");
                if manifest_path.is_file() {
                    push_project(&manifest_path, &mut projects, &mut seen_manifests)?;
                }
            }
        }
    }

    Ok(projects)
}

fn build_exclusion_patterns(root: &Path, packages: &[String]) -> Result<Vec<glob::Pattern>> {
    let mut exclusions = Vec::new();

    for pattern in packages {
        let Some(exclusion) = pattern.strip_prefix('!') else {
            continue;
        };

        let absolute = root.join(exclusion);
        let compiled = glob::Pattern::new(&absolute.to_string_lossy()).map_err(|error| {
            SnpmError::WorkspaceConfig {
                path: root.to_path_buf(),
                reason: error.to_string(),
            }
        })?;

        exclusions.push(compiled);
    }

    Ok(exclusions)
}

fn is_excluded(path: &Path, exclusions: &[glob::Pattern]) -> bool {
    let path = path.to_string_lossy();
    exclusions
        .iter()
        .any(|pattern| pattern.matches(path.as_ref()))
}

fn push_project(
    manifest_path: &Path,
    projects: &mut Vec<Project>,
    seen_manifests: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let manifest_path = manifest_path.to_path_buf();
    if seen_manifests.insert(manifest_path.clone()) {
        projects.push(Project::from_manifest_path(manifest_path)?);
    }

    Ok(())
}
