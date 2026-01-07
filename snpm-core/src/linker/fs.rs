use crate::{LinkBackend, Result, SnpmConfig, SnpmError};
use std::fs;
use std::path::Path;

pub fn link_dir(config: &SnpmConfig, source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|source_err| SnpmError::WriteFile {
        path: dest.to_path_buf(),
        source: source_err,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| SnpmError::ReadFile {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source_err| SnpmError::ReadFile {
                path: entry.path(),
                source: source_err,
            })?;

        let from = entry.path();
        let to = dest.join(entry.file_name());

        if file_type.is_dir() {
            link_dir(config, &from, &to)?;
        } else {
            link_file(config, &from, &to)?;
        }
    }

    Ok(())
}

fn link_file(config: &SnpmConfig, from: &Path, to: &Path) -> Result<()> {
    match config.link_backend {
        LinkBackend::Auto => {
            if fs::hard_link(from, to).is_ok() {
                return Ok(());
            }

            if symlink_file_entry(from, to).is_ok() {
                return Ok(());
            }

            fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                path: to.to_path_buf(),
                source: source_err,
            })?;
        }
        LinkBackend::Hardlink => {
            if fs::hard_link(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Symlink => {
            if symlink_file_entry(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Copy => {
            fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                path: to.to_path_buf(),
                source: source_err,
            })?;
        }
    }

    Ok(())
}

pub fn copy_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|source_err| SnpmError::WriteFile {
        path: dest.to_path_buf(),
        source: source_err,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| SnpmError::ReadFile {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source_err| SnpmError::ReadFile {
                path: entry.path(),
                source: source_err,
            })?;

        let from = entry.path();
        let to = dest.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|source_err| SnpmError::WriteFile {
                path: to,
                source: source_err,
            })?;
        }
    }

    Ok(())
}

#[cfg(unix)]
pub fn symlink_dir_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(from, to)
}

#[cfg(windows)]
pub fn symlink_dir_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_dir;
    symlink_dir(from, to)
}

#[cfg(unix)]
pub fn symlink_file_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(from, to)
}

#[cfg(windows)]
pub fn symlink_file_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_file;
    symlink_file(from, to)
}
