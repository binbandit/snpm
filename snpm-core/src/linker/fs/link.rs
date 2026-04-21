use super::paths::ensure_parent_dir;
use super::symlinks::symlink_file_entry;
use crate::store::read_package_filesystem_shape_lossy;
use crate::{LinkBackend, Result, SnpmConfig, SnpmError};

use rayon::prelude::*;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static RESOLVED_AUTO_BACKEND: OnceLock<LinkBackend> = OnceLock::new();

pub fn link_dir(config: &SnpmConfig, source: &Path, dest: &Path) -> Result<()> {
    if try_clone_store_package_dir(config, source, dest)? {
        return Ok(());
    }

    let (directories, files) = match indexed_link_ops(source, dest) {
        Some(ops) => ops,
        None => {
            let mut directories = vec![dest.to_path_buf()];
            let mut files = Vec::new();
            collect_link_ops(source, dest, &mut directories, &mut files)?;
            (directories, files)
        }
    };

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

fn should_try_clone_store_package_dir(config: &SnpmConfig, source: &Path) -> bool {
    matches!(
        config.link_backend,
        LinkBackend::Auto | LinkBackend::Reflink
    ) && source.starts_with(config.packages_dir())
}

fn try_clone_store_package_dir(config: &SnpmConfig, source: &Path, dest: &Path) -> Result<bool> {
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos"
    ))]
    {
        if !should_try_clone_store_package_dir(config, source) {
            return Ok(false);
        }

        ensure_parent_dir(dest)?;

        return match reflink_copy::reflink(source, dest) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        };
    }

    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "tvos",
        target_os = "watchos"
    )))]
    {
        let _ = (config, source, dest);
        Ok(false)
    }
}

fn indexed_link_ops(source: &Path, dest: &Path) -> Option<(Vec<PathBuf>, Vec<(PathBuf, PathBuf)>)> {
    let shape = read_package_filesystem_shape_lossy(source)?;
    let mut directories = Vec::with_capacity(shape.directories.len() + 1);
    directories.push(dest.to_path_buf());
    directories.extend(shape.directories.iter().map(|relative| dest.join(relative)));

    let files = shape
        .files
        .iter()
        .map(|relative| (source.join(relative), dest.join(relative)))
        .collect();

    Some((directories, files))
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
    let requested_backend = config.link_backend;
    let backend = match requested_backend {
        LinkBackend::Auto => resolve_auto_backend(from, to),
        other => other,
    };

    match backend {
        LinkBackend::Auto => unreachable!(),
        LinkBackend::Reflink => {
            if reflink_copy::reflink(from, to).is_err() {
                if matches!(requested_backend, LinkBackend::Auto) && fs::hard_link(from, to).is_ok()
                {
                    return Ok(());
                }
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

#[cfg(test)]
mod tests {
    use super::{link_dir, should_try_clone_store_package_dir};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::store::PACKAGE_METADATA_FILE;

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::None,
            link_backend: LinkBackend::Copy,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn link_dir_uses_indexed_shape_without_scanning_source_tree() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        fs::create_dir_all(source.join("bin")).unwrap();
        fs::write(source.join("bin/tool.js"), "console.log('ok');").unwrap();
        fs::write(
            source.join(PACKAGE_METADATA_FILE),
            r#"{
                "filesystem": {
                    "directories": ["bin"],
                    "files": ["bin/tool.js", ".snpm-package-metadata.json"]
                }
            }"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), source.join("bad-link")).unwrap();

        link_dir(&make_config(), &source, &destination).unwrap();

        assert!(destination.join("bin/tool.js").is_file());
        assert!(destination.join(PACKAGE_METADATA_FILE).is_file());
    }

    #[test]
    fn store_clone_gate_only_allows_store_paths_for_auto_and_reflink() {
        let dir = tempdir().unwrap();
        let mut config = make_config();
        config.data_dir = dir.path().join("data");
        config.link_backend = LinkBackend::Auto;

        let store_path = config.packages_dir().join("dep").join("1.0.0");
        let external_path = dir.path().join("external");

        assert!(should_try_clone_store_package_dir(&config, &store_path));
        assert!(!should_try_clone_store_package_dir(&config, &external_path));

        config.link_backend = LinkBackend::Reflink;
        assert!(should_try_clone_store_package_dir(&config, &store_path));

        config.link_backend = LinkBackend::Hardlink;
        assert!(!should_try_clone_store_package_dir(&config, &store_path));

        config.link_backend = LinkBackend::Copy;
        assert!(!should_try_clone_store_package_dir(&config, &store_path));
    }

    #[cfg(unix)]
    #[test]
    fn copy_backend_does_not_alias_source_inode() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let destination = dir.path().join("destination");
        let file = source.join("file.txt");

        fs::create_dir_all(&source).unwrap();
        fs::write(&file, "hello").unwrap();

        link_dir(&make_config(), &source, &destination).unwrap();

        let source_inode = fs::metadata(&file).unwrap().ino();
        let destination_inode = fs::metadata(destination.join("file.txt")).unwrap().ino();
        assert_ne!(source_inode, destination_inode);
    }
}
