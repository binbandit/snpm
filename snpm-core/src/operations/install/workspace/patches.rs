use crate::console;
use crate::operations::patch as patch_ops;
use crate::patch;
use crate::resolve::PackageId;
use crate::{Result, SnpmError, Workspace};

use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) fn apply_workspace_patches(
    workspace: &Workspace,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<usize> {
    let mut patches_to_apply = BTreeMap::<(String, String), (PathBuf, PathBuf)>::new();

    for project in &workspace.projects {
        for (name, version, patch_path) in patch_ops::get_patches_to_apply(project)? {
            let safe_name = name.replace('/', "+");
            let package_dir = project
                .root
                .join("node_modules")
                .join(".snpm")
                .join(format!("{}@{}", safe_name, version))
                .join("node_modules")
                .join(&name);

            if !package_dir.exists() {
                console::warn(&format!(
                    "Patch for {}@{} skipped: package not installed in {}",
                    name,
                    version,
                    project.root.display()
                ));
                continue;
            }

            let key = (name.clone(), version.clone());
            if let Some((existing_patch, _)) = patches_to_apply.get(&key) {
                if existing_patch != &patch_path {
                    return Err(SnpmError::WorkspaceConfig {
                        path: workspace.root.clone(),
                        reason: format!(
                            "conflicting patches configured for {}@{} across workspace projects",
                            name, version
                        ),
                    });
                }
                continue;
            }

            patches_to_apply.insert(key, (patch_path, package_dir));
        }
    }

    let mut applied = 0;

    for ((name, version), (patch_path, package_dir)) in patches_to_apply {
        let package_id = PackageId {
            name: name.clone(),
            version: version.clone(),
        };
        let Some(store_path) = store_paths.get(&package_id) else {
            console::warn(&format!(
                "Patch for {}@{} skipped: package missing from cache graph",
                name, version
            ));
            continue;
        };

        patch::materialize_patch_target(&package_dir, store_path)?;

        match patch::apply_patch(&package_dir, &patch_path) {
            Ok(()) => applied += 1,
            Err(error) => {
                console::warn(&format!(
                    "Failed to apply patch for {}@{}: {}",
                    name, version, error
                ));
            }
        }
    }

    Ok(applied)
}
