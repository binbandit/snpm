mod defaults;
mod manifest;
mod walk;

use crate::{Project, Result};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::PackFileReason;
use defaults::collect_default_files;
use manifest::{add_bin_entries, add_main_entry, add_mandatory_files, collect_manifest_files};

#[derive(Debug, Clone)]
pub(super) struct CollectedFile {
    pub(super) absolute_path: PathBuf,
    pub(super) relative_path: String,
    pub(super) size: u64,
    pub(super) reason: PackFileReason,
}

pub(super) fn collect_pack_files(project: &Project) -> Result<Vec<CollectedFile>> {
    let mut files = BTreeMap::new();
    add_file(
        &project.root,
        &project.manifest_path,
        PackFileReason::Manifest,
        &mut files,
    )?;

    if project.manifest.files.is_some() {
        collect_manifest_files(project, &mut files)?;
    } else {
        collect_default_files(project, &mut files)?;
    }

    add_mandatory_files(&project.root, &mut files)?;
    add_main_entry(&project.root, &mut files, project);
    add_bin_entries(&project.root, &mut files, project);

    Ok(files.into_values().collect())
}

pub(super) fn add_file(
    root: &Path,
    path: &Path,
    reason: PackFileReason,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
) -> Result<()> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|source| crate::SnpmError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;

    if !metadata.file_type().is_file() {
        return Ok(());
    }

    files
        .entry(path.to_path_buf())
        .or_insert_with(|| CollectedFile {
            absolute_path: path.to_path_buf(),
            relative_path: to_relative_path(root, path),
            size: metadata.len(),
            reason,
        });

    Ok(())
}

fn to_relative_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}
