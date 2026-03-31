use crate::linker::fs::ensure_parent_dir;
use crate::{Project, Result, SnpmError, Workspace};

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn link_local_workspace_deps(
    project: &Project,
    workspace: Option<&Workspace>,
    local_deps: &BTreeSet<String>,
    local_dev_deps: &BTreeSet<String>,
    local_optional_deps: &BTreeSet<String>,
    include_dev: bool,
) -> Result<()> {
    if local_deps.is_empty() && local_dev_deps.is_empty() && local_optional_deps.is_empty() {
        return Ok(());
    }

    let workspace_reference = workspace.ok_or_else(|| SnpmError::WorkspaceConfig {
        path: project.root.clone(),
        reason: "workspace: protocol used but no workspace configuration found".into(),
    })?;
    let node_modules = project.root.join("node_modules");

    for name in local_deps
        .iter()
        .chain(local_dev_deps.iter())
        .chain(local_optional_deps.iter())
    {
        let only_dev = local_dev_deps.contains(name) && !local_deps.contains(name);
        if !include_dev && only_dev {
            continue;
        }

        let source_project = workspace_reference
            .projects
            .iter()
            .find(|candidate| candidate.manifest.name.as_deref() == Some(name.as_str()))
            .ok_or_else(|| SnpmError::WorkspaceConfig {
                path: workspace_reference.root.clone(),
                reason: format!("workspace dependency {name} not found in workspace projects"),
            })?;

        let destination = node_modules.join(name);
        ensure_parent_dir(&destination)?;
        remove_existing_destination(&destination)?;
        symlink_workspace_project(&source_project.root, &destination)?;
    }

    Ok(())
}

fn remove_existing_destination(destination: &Path) -> Result<()> {
    if !destination.exists() {
        return Ok(());
    }

    let remove = if destination.is_dir() {
        fs::remove_dir_all(destination)
    } else {
        fs::remove_file(destination)
    };

    remove.map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn symlink_workspace_project(source: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;

    symlink(source, destination).map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}

#[cfg(windows)]
fn symlink_workspace_project(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::fs::symlink_dir;

    symlink_dir(source, destination).map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}
