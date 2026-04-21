use crate::copying::clone_or_copy_file;
use crate::{Result, SnpmError};

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub(crate) fn reset_package_dir(target_dir: &Path) -> Result<()> {
    match fs::remove_dir_all(target_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(source) => {
            return Err(SnpmError::WriteFile {
                path: target_dir.to_path_buf(),
                source,
            });
        }
    }

    fs::create_dir_all(target_dir).map_err(|source| SnpmError::WriteFile {
        path: target_dir.to_path_buf(),
        source,
    })
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
