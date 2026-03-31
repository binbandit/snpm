use super::paths::ensure_parent_dir;
use super::symlinks::symlink_file_entry;
use crate::{LinkBackend, Result, SnpmConfig, SnpmError};

use rayon::prelude::*;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static RESOLVED_AUTO_BACKEND: OnceLock<LinkBackend> = OnceLock::new();

pub fn link_dir(config: &SnpmConfig, source: &Path, dest: &Path) -> Result<()> {
    let mut directories = vec![dest.to_path_buf()];
    let mut files = Vec::new();
    collect_link_ops(source, dest, &mut directories, &mut files)?;

    for directory in &directories {
        fs::create_dir_all(directory).map_err(|source_err| SnpmError::WriteFile {
            path: directory.clone(),
            source: source_err,
        })?;
    }

    files
        .par_iter()
        .try_for_each(|(from, to)| link_file(config, from, to))
}

fn collect_link_ops(
    source: &Path,
    dest: &Path,
    directories: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<()> {
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

        if file_type.is_symlink() {
            return Err(SnpmError::Io {
                path: from,
                source: std::io::Error::new(
                    ErrorKind::InvalidData,
                    "refusing to link symlink from package store",
                ),
            });
        }

        if file_type.is_dir() {
            directories.push(to.clone());
            collect_link_ops(&from, &to, directories, files)?;
        } else {
            files.push((from, to));
        }
    }

    Ok(())
}

fn resolve_auto_backend(from: &Path, to: &Path) -> LinkBackend {
    *RESOLVED_AUTO_BACKEND.get_or_init(|| {
        if reflink_copy::reflink(from, to).is_ok() {
            let _ = fs::remove_file(to);
            return LinkBackend::Reflink;
        }

        if fs::hard_link(from, to).is_ok() {
            let _ = fs::remove_file(to);
            return LinkBackend::Hardlink;
        }

        if symlink_file_entry(from, to).is_ok() {
            let _ = fs::remove_file(to);
            return LinkBackend::Symlink;
        }

        LinkBackend::Copy
    })
}

fn link_file(config: &SnpmConfig, from: &Path, to: &Path) -> Result<()> {
    let backend = match config.link_backend {
        LinkBackend::Auto => resolve_auto_backend(from, to),
        other => other,
    };

    match backend {
        LinkBackend::Auto => unreachable!(),
        LinkBackend::Reflink => {
            if reflink_copy::reflink(from, to).is_err() {
                copy_file(from, to)?;
            }
        }
        LinkBackend::Hardlink => {
            if fs::hard_link(from, to).is_err() {
                copy_file(from, to)?;
            }
        }
        LinkBackend::Symlink => {
            if symlink_file_entry(from, to).is_err() {
                copy_file(from, to)?;
            }
        }
        LinkBackend::Copy => copy_file(from, to)?,
    }

    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<()> {
    ensure_parent_dir(to)?;
    fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
        path: to.to_path_buf(),
        source: source_err,
    })?;
    Ok(())
}
