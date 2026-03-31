use crate::patch::{get_patched_dependencies, parse_patch_key, patches_dir};
use crate::{Project, Result, SnpmError};

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub(super) fn collect_patches_to_apply(
    project: &Project,
) -> Result<Vec<(String, String, PathBuf)>> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();

    for (key, rel_path) in get_patched_dependencies(project) {
        if let Some((name, version)) = parse_patch_key(&key) {
            let patch_path = project.root.join(&rel_path);
            if patch_path.exists() {
                seen.insert(format!("{}@{}", name, version));
                result.push((name, version, patch_path));
            }
        }
    }

    let patches = patches_dir(project);
    if !patches.exists() {
        return Ok(result);
    }

    for entry in fs::read_dir(&patches)
        .map_err(|source| SnpmError::ReadFile {
            path: patches.clone(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let filename = match path.file_name().and_then(|name| name.to_str()) {
            Some(name) if name.ends_with(".patch") => name,
            _ => continue,
        };

        let name_version = match filename.strip_suffix(".patch") {
            Some(name_version) => name_version,
            None => continue,
        };

        if let Some((name, version)) = parse_patch_key(name_version) {
            let package_name = name.replace('+', "/");
            let key = format!("{}@{}", package_name, version);

            if !seen.contains(&key) {
                result.push((package_name, version, path));
            }
        }
    }

    Ok(result)
}
