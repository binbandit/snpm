use crate::linker::fs::{ensure_parent_dir, symlink_is_correct};
use crate::{Project, Result, SnpmError, Workspace};

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
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

        let source_project = workspace_reference.project_by_name(name).ok_or_else(|| {
            SnpmError::WorkspaceConfig {
                path: workspace_reference.root.clone(),
                reason: format!("workspace dependency {name} not found in workspace projects"),
            }
        })?;

        let destination = node_modules.join(name);
        if symlink_is_correct(&destination, &source_project.root) {
            continue;
        }

        ensure_parent_dir(&destination)?;
        remove_existing_destination(&destination)?;
        symlink_workspace_project(&source_project.root, &destination)?;
    }

    Ok(())
}

fn remove_existing_destination(destination: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(SnpmError::WriteFile {
                path: destination.to_path_buf(),
                source,
            });
        }
    };

    let remove = if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
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

#[cfg(test)]
mod tests {
    use super::{link_local_workspace_deps, symlink_workspace_project};
    use crate::Project;
    use crate::project::Manifest;
    use crate::workspace::types::{Workspace, WorkspaceConfig};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_project(root: PathBuf, name: &str) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some(name.to_string()),
                version: Some("1.0.0".to_string()),
                private: false,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        }
    }

    #[cfg(unix)]
    #[test]
    fn link_local_workspace_deps_keeps_existing_correct_link() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempdir().unwrap();
        let app_root = dir.path().join("app");
        let dep_root = dir.path().join("dep");
        let destination = app_root.join("node_modules/dep");

        fs::create_dir_all(&app_root).unwrap();
        fs::create_dir_all(&dep_root).unwrap();
        fs::create_dir_all(app_root.join("node_modules")).unwrap();
        symlink_workspace_project(&dep_root, &destination).unwrap();

        let project = make_project(app_root.clone(), "app");
        let workspace = Workspace {
            root: dir.path().to_path_buf(),
            projects: vec![make_project(dep_root.clone(), "dep")],
            config: WorkspaceConfig {
                packages: vec!["*".to_string()],
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: Vec::new(),
                ignored_built_dependencies: Vec::new(),
                hoisting: None,
            },
        };

        let before = fs::symlink_metadata(&destination).unwrap().ino();
        link_local_workspace_deps(
            &project,
            Some(&workspace),
            &BTreeSet::from(["dep".to_string()]),
            &BTreeSet::new(),
            &BTreeSet::new(),
            true,
        )
        .unwrap();
        let after = fs::symlink_metadata(&destination).unwrap().ino();

        assert_eq!(before, after);
    }
}
