use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

pub(in crate::linker::bins) fn create_bin_file(
    bin_dir: &Path,
    name: &str,
    target: &Path,
) -> Result<()> {
    let destination = bin_dir.join(name);
    super::super::fs::ensure_parent_dir(&destination)?;

    if !target.is_file() {
        remove_existing_bin_entry(&destination)?;
        return Ok(());
    }

    ensure_bin_target_executable(target)?;

    if super::super::fs::symlink_is_correct(&destination, target) {
        return Ok(());
    }

    remove_existing_bin_entry(&destination)?;
    write_bin_link(target, &destination)
}

fn remove_existing_bin_entry(path: &Path) -> Result<()> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path).map_err(|source| SnpmError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    } else {
        fs::remove_file(path).map_err(|source| SnpmError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    }

    Ok(())
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

    #[cfg(unix)]
    #[test]
    fn create_bin_file_keeps_existing_correct_symlink() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempdir().unwrap();
        let target = dir.path().join("cli.js");
        let bin_dir = dir.path().join(".bin");

        fs::write(&target, "#!/usr/bin/env node\n").unwrap();

        create_bin_file(&bin_dir, "tool", &target).unwrap();
        let destination = bin_dir.join("tool");
        let before = fs::symlink_metadata(&destination).unwrap().ino();

        create_bin_file(&bin_dir, "tool", &target).unwrap();
        let after = fs::symlink_metadata(&destination).unwrap().ino();

        assert_eq!(before, after);
    }

    #[test]
    fn create_bin_file_removes_stale_destination_when_target_is_missing() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("missing.js");
        let bin_dir = dir.path().join(".bin");
        let destination = bin_dir.join("tool");

        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(&destination, "stale").unwrap();

        create_bin_file(&bin_dir, "tool", &target).unwrap();

        assert!(!destination.exists());
        assert!(destination.symlink_metadata().is_err());
    }
}
