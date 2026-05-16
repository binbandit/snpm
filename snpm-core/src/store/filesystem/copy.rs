use crate::copying::clone_or_copy_file;
use crate::{Result, SnpmError};

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Move `staged` into `final_path` atomically once extraction has succeeded.
///
/// Behavior:
/// - Happy path: `fs::rename` succeeds when `final_path` is missing.
/// - If another worker already finished (marker file is present), discard our
///   staging directory and report success.
/// - If a partial extract from a prior crash or concurrent racer occupies
///   `final_path`, clear it and retry the rename so our complete tree wins.
/// - If `final_path` is something other than a directory (a stray file or
///   symlink), it is also removed before retrying the rename.
pub(crate) fn atomic_finalize_extracted_dir(staged: &Path, final_path: &Path) -> Result<()> {
    if let Ok(()) = fs::rename(staged, final_path) {
        return Ok(());
    }

    if final_path.join(".snpm_complete").is_file() {
        let _ = fs::remove_dir_all(staged);
        return Ok(());
    }

    remove_path_any(final_path);
    fs::rename(staged, final_path).map_err(|source| {
        let _ = fs::remove_dir_all(staged);
        SnpmError::WriteFile {
            path: final_path.to_path_buf(),
            source,
        }
    })
}

fn remove_path_any(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_dir() && !file_type.is_symlink() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
        Err(_) => {}
    }
}

pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let name = entry.file_name();

        if name == ".git" || name == "node_modules" {
            continue;
        }

        if entry_type.is_symlink() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "refusing to copy symlink from local dependency: {}",
                    entry.path().display()
                ),
            ));
        }

        let destination = dst.join(&name);
        if entry_type.is_dir() {
            copy_dir_all(&entry.path(), &destination)?;
        } else {
            clone_or_copy_file(&entry.path(), &destination)?;
        }
    }

    Ok(())
}
