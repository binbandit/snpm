use crate::{Project, Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

use super::walk::collect_dir_files;

pub(super) fn collect_manifest_files(project: &Project, files: &mut Vec<PathBuf>) -> Result<()> {
    let root = &project.root;

    for pattern in project.manifest.files.as_ref().into_iter().flatten() {
        let full_pattern = root.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        match glob::glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    add_pattern_match(&entry, &project.manifest_path, files)?;
                }
            }
            Err(_) => {
                add_pattern_match(&root.join(pattern), &project.manifest_path, files)?;
            }
        }
    }

    Ok(())
}

pub(super) fn add_mandatory_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
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
            if name_str.to_uppercase().starts_with(mandatory)
                && entry.path().is_file()
                && !files.contains(&entry.path())
            {
                files.push(entry.path());
            }
        }
    }

    Ok(())
}

pub(super) fn add_main_entry(root: &Path, files: &mut Vec<PathBuf>, project: &Project) {
    if let Some(main) = project.manifest.main.as_ref() {
        let main_path = root.join(main);
        if main_path.is_file() && !files.contains(&main_path) {
            files.push(main_path);
        }
    }
}

fn add_pattern_match(path: &Path, manifest_path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_file() && path != manifest_path {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        collect_dir_files(path, files)?;
    }

    Ok(())
}
