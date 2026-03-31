use crate::project::ManifestSnpm;
use crate::{Project, Result};

use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn update_manifest_with_patch(
    project: &Project,
    package_name: &str,
    package_version: &str,
    patch_path: &Path,
) -> Result<()> {
    let mut manifest = project.manifest.clone();

    let key = format!("{}@{}", package_name, package_version);
    let rel_path = patch_path
        .strip_prefix(&project.root)
        .unwrap_or(patch_path)
        .to_string_lossy()
        .to_string();

    let snpm = manifest.snpm.get_or_insert_with(|| ManifestSnpm {
        overrides: BTreeMap::new(),
        patched_dependencies: None,
    });

    snpm.patched_dependencies
        .get_or_insert_with(BTreeMap::new)
        .insert(key, rel_path);

    project.write_manifest(&manifest)
}

pub(super) fn remove_patch_from_manifest(project: &Project, package_name: &str) -> Result<()> {
    let mut manifest = project.manifest.clone();
    let mut modified = false;

    modified |= remove_from_patched_deps(&mut manifest.snpm, package_name);
    modified |= remove_from_patched_deps(&mut manifest.pnpm, package_name);

    if modified {
        project.write_manifest(&manifest)?;
    }

    Ok(())
}

fn remove_from_patched_deps<T: HasPatchedDependencies>(
    config: &mut Option<T>,
    package_name: &str,
) -> bool {
    let config = match config.as_mut() {
        Some(config) => config,
        None => return false,
    };

    let patched = match config.patched_dependencies_mut() {
        Some(patched) => patched,
        None => return false,
    };

    let keys_to_remove: Vec<_> = patched
        .keys()
        .filter(|key| matches_package_name(key, package_name))
        .cloned()
        .collect();

    let removed_any = !keys_to_remove.is_empty();

    for key in keys_to_remove {
        patched.remove(&key);
    }

    if patched.is_empty() {
        config.clear_patched_dependencies();
    }

    removed_any
}

fn matches_package_name(key: &str, package_name: &str) -> bool {
    crate::patch::parse_patch_key(key)
        .map(|(name, _)| name == package_name || name.replace('+', "/") == package_name)
        .unwrap_or(false)
}

trait HasPatchedDependencies {
    fn patched_dependencies_mut(&mut self) -> Option<&mut BTreeMap<String, String>>;
    fn clear_patched_dependencies(&mut self);
}

impl HasPatchedDependencies for crate::project::ManifestSnpm {
    fn patched_dependencies_mut(&mut self) -> Option<&mut BTreeMap<String, String>> {
        self.patched_dependencies.as_mut()
    }

    fn clear_patched_dependencies(&mut self) {
        self.patched_dependencies = None;
    }
}

impl HasPatchedDependencies for crate::project::ManifestPnpm {
    fn patched_dependencies_mut(&mut self) -> Option<&mut BTreeMap<String, String>> {
        self.patched_dependencies.as_mut()
    }

    fn clear_patched_dependencies(&mut self) {
        self.patched_dependencies = None;
    }
}
