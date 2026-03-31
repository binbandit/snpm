use crate::{Project, Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

use super::super::types::{PatchInfo, parse_patch_key, patches_dir};

pub fn remove_patch(project: &Project, package_name: &str) -> Result<Option<PathBuf>> {
    let patches = patches_dir(project);
    if !patches.exists() {
        return Ok(None);
    }

    let prefix = patch_prefix(package_name);

    for entry in fs::read_dir(&patches)
        .map_err(|source| SnpmError::ReadFile {
            path: patches.clone(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let Some(filename) = patch_filename(&path) else {
            continue;
        };

        if filename.starts_with(&prefix) && filename.ends_with(".patch") {
            fs::remove_file(&path).map_err(|source| SnpmError::Io {
                path: path.clone(),
                source,
            })?;
            return Ok(Some(path));
        }
    }

    Ok(None)
}

pub fn list_patches(project: &Project) -> Result<Vec<PatchInfo>> {
    let patches = patches_dir(project);
    if !patches.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();

    for entry in fs::read_dir(&patches)
        .map_err(|source| SnpmError::ReadFile {
            path: patches.clone(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let Some(name_version) = patch_filename(&path).and_then(|name| name.strip_suffix(".patch"))
        else {
            continue;
        };

        if let Some((name, version)) = parse_patch_key(name_version) {
            result.push(PatchInfo {
                package_name: name.replace('+', "/"),
                package_version: version,
                patch_path: path,
            });
        }
    }

    Ok(result)
}

fn patch_prefix(package_name: &str) -> String {
    format!("{}@", package_name.replace('/', "+"))
}

fn patch_filename(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}
