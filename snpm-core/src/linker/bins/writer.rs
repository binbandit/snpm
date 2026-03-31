use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

pub(in crate::linker::bins) fn create_bin_file(
    bin_dir: &Path,
    name: &str,
    target: &Path,
) -> Result<()> {
    if !target.is_file() {
        return Ok(());
    }

    let destination = bin_dir.join(name);
    super::super::fs::ensure_parent_dir(&destination)?;

    if destination.exists() {
        fs::remove_file(&destination).map_err(|source| SnpmError::WriteFile {
            path: destination.clone(),
            source,
        })?;
    }

    write_bin_link(target, &destination)
}

#[cfg(unix)]
fn write_bin_link(target: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;

    if let Err(_source) = symlink(target, destination) {
        fs::copy(target, destination).map_err(|source| SnpmError::WriteFile {
            path: destination.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}

#[cfg(windows)]
fn write_bin_link(target: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::fs::symlink_file;

    if let Err(_source) = symlink_file(target, destination) {
        fs::copy(target, destination).map_err(|source| SnpmError::WriteFile {
            path: destination.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}
