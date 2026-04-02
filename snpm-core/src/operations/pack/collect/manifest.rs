use crate::project::BinField;
use crate::{Project, Result, SnpmError};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::PackFileReason;
use super::walk::{collect_dir_files_filtered_from_root, should_include_file_from_root};
use super::{CollectedFile, add_file};

pub(super) fn collect_manifest_files(
    project: &Project,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
) -> Result<()> {
    let root = &project.root;

    for pattern in project.manifest.files.as_ref().into_iter().flatten() {
        let full_pattern = root.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        match glob::glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    add_pattern_match(&entry, root, &project.manifest_path, files)?;
                }
            }
            Err(_) => {
                add_pattern_match(&root.join(pattern), root, &project.manifest_path, files)?;
            }
        }
    }

    Ok(())
}

pub(super) fn add_mandatory_files(
    root: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
) -> Result<()> {
    for mandatory in &["README", "LICENSE", "LICENCE", "CHANGELOG"] {
        for entry in fs::read_dir(root)
            .map_err(|source| SnpmError::ReadFile {
                path: root.to_path_buf(),
                source,
            })?
            .flatten()
        {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.to_uppercase().starts_with(mandatory) && entry.path().is_file() {
                add_file(root, &entry.path(), PackFileReason::Mandatory, files)?;
            }
        }
    }

    Ok(())
}

pub(super) fn add_main_entry(
    root: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
    project: &Project,
) {
    if let Some(main) = project.manifest.main.as_ref() {
        let main_path = root.join(main);
        if main_path.is_file() {
            add_file(root, &main_path, PackFileReason::MainEntry, files).ok();
        }
    }
}

pub(super) fn add_bin_entries(
    root: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
    project: &Project,
) {
    match project.manifest.bin.as_ref() {
        Some(BinField::Single(path)) => {
            add_file(root, &root.join(path), PackFileReason::BinEntry, files).ok();
        }
        Some(BinField::Map(entries)) => {
            for path in entries.values() {
                add_file(root, &root.join(path), PackFileReason::BinEntry, files).ok();
            }
        }
        None => {}
    }
}

fn add_pattern_match(
    path: &Path,
    root: &Path,
    manifest_path: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
) -> Result<()> {
    if path.is_file() && path != manifest_path && should_include_file_from_root(path, root, false)?
    {
        add_file(root, path, PackFileReason::ManifestFiles, files)?;
    } else if path.is_dir() {
        collect_dir_files_filtered_from_root(
            path,
            files,
            root,
            false,
            PackFileReason::ManifestFiles,
        )?;
    }

    Ok(())
}
