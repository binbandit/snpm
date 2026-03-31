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

    ensure_bin_target_executable(target)?;

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
        ensure_executable_bits(destination)?;
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

#[cfg(unix)]
fn ensure_bin_target_executable(path: &Path) -> Result<()> {
    ensure_executable_bits(path)
}

#[cfg(windows)]
fn ensure_bin_target_executable(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn ensure_executable_bits(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let mut permissions = metadata.permissions();
    let mode = permissions.mode();
    let executable_mode = mode | ((mode & 0o444) >> 2);

    if executable_mode == mode {
        return Ok(());
    }

    permissions.set_mode(executable_mode);
    fs::set_permissions(path, permissions).map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::create_bin_file;

    use std::fs;
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn create_bin_file_marks_target_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let target = dir.path().join("cli.js");
        let bin_dir = dir.path().join(".bin");

        fs::write(&target, "#!/usr/bin/env node\n").unwrap();

        let mut permissions = fs::metadata(&target).unwrap().permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&target, permissions).unwrap();

        create_bin_file(&bin_dir, "tool", &target).unwrap();

        let updated_mode = fs::metadata(&target).unwrap().permissions().mode();
        assert_eq!(updated_mode & 0o111, 0o111);
        assert!(bin_dir.join("tool").exists());
    }
}
