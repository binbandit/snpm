use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

pub(super) fn replace_path(path: &Path) {
    if path.symlink_metadata().is_err() {
        return;
    }

    let is_real_directory = path.is_dir()
        && !path
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink());

    if is_real_directory {
        fs::remove_dir_all(path).ok();
    } else {
        fs::remove_file(path).ok();
    }
}

pub(super) fn create_symlink(source: &Path, dest: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, dest).map_err(|source_err| SnpmError::WriteFile {
            path: dest.to_path_buf(),
            source: source_err,
        })?;
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, dest).map_err(|source_err| {
            SnpmError::WriteFile {
                path: dest.to_path_buf(),
                source: source_err,
            }
        })?;
    }

    Ok(())
}
