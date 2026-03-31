use crate::{Project, Result, SnpmError, Workspace};

use sha2::{Digest, Sha256};
use std::fs;

use super::lockfile::NO_PATCH_HASH;

pub fn compute_project_patch_hash(project: &Project) -> Result<String> {
    let patched_dependencies = crate::patch::get_patched_dependencies(project);
    if patched_dependencies.is_empty() {
        return Ok(NO_PATCH_HASH.to_string());
    }

    let mut hasher = Sha256::new();

    for (key, rel_path) in patched_dependencies {
        hasher.update(key.as_bytes());
        hasher.update(rel_path.as_bytes());

        let patch_path = project.root.join(&rel_path);
        if patch_path.is_file() {
            let bytes = fs::read(&patch_path).map_err(|source| SnpmError::ReadFile {
                path: patch_path,
                source,
            })?;
            hasher.update(&bytes);
        } else {
            hasher.update(b"__missing__");
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn compute_workspace_patch_hash(workspace: &Workspace) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut has_any_patches = false;
    let mut projects: Vec<&Project> = workspace.projects.iter().collect();
    projects.sort_by(|left, right| left.root.cmp(&right.root));

    for project in projects {
        let patch_hash = compute_project_patch_hash(project)?;
        if patch_hash != NO_PATCH_HASH {
            has_any_patches = true;
        }

        hasher.update(project.root.display().to_string().as_bytes());
        hasher.update(patch_hash.as_bytes());
    }

    if has_any_patches {
        Ok(format!("{:x}", hasher.finalize()))
    } else {
        Ok(NO_PATCH_HASH.to_string())
    }
}
