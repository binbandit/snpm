use crate::console;
use crate::operations::patch as patch_ops;
use crate::patch;
use crate::resolve::PackageId;
use crate::{Project, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) fn apply_patches(
    project: &Project,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<usize> {
    let patches = patch_ops::get_patches_to_apply(project)?;
    if patches.is_empty() {
        return Ok(0);
    }

    let mut applied = 0;

    for (name, version, patch_path) in &patches {
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

        let safe_name = name.replace('/', "+");
        let package_dir = project
            .root
            .join("node_modules")
            .join(".snpm")
            .join(format!("{}@{}", safe_name, version))
            .join("node_modules")
            .join(name);

        if !package_dir.exists() {
            console::warn(&format!(
                "Patch for {}@{} skipped: package not installed",
                name, version
            ));
            continue;
        }

        console::verbose(&format!(
            "applying patch for {}@{} from {}",
            name,
            version,
            patch_path.display()
        ));

        patch::materialize_patch_target(&package_dir, store_path)?;

        match patch::apply_patch(&package_dir, patch_path) {
            Ok(()) => {
                console::step(&format!("Applied patch for {}@{}", name, version));
                applied += 1;
            }
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
