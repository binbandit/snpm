use crate::copying::clone_or_copy_file;
use crate::linker::fs::{symlink_dir_entry, symlink_file_entry};
use crate::{Result, SnpmConfig, SnpmError};

use sha2::{Digest, Sha512};
use std::fs;
use std::path::{Path, PathBuf};

const SIDE_EFFECTS_CACHE_MARKER: &str = ".snpm-side-effects-cache";
const SIDE_EFFECTS_TMP_PREFIX: &str = ".tmp-side-effects-";

pub(super) enum SideEffectsCacheRestore {
    Miss,
    Restored,
    AlreadyApplied,
}

pub(super) struct SideEffectsCacheEntry {
    input_hash: String,
    path: PathBuf,
}

impl SideEffectsCacheEntry {
    pub(super) fn new(
        config: &SnpmConfig,
        name: &str,
        version: &str,
        package_dir: &Path,
    ) -> Result<Self> {
        let input_hash = match read_marker(package_dir) {
            Some(hash) => hash,
            None => hash_dir(package_dir)?,
        };
        let safe_name = name.replace('/', "__");
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);

        Ok(Self {
            path: config
                .side_effects_cache_dir()
                .join(platform)
                .join(format!("{safe_name}@{version}"))
                .join(&input_hash),
            input_hash,
        })
    }

    pub(super) fn restore_if_available(
        &self,
        package_dir: &Path,
    ) -> Result<SideEffectsCacheRestore> {
        if marker_matches(package_dir, &self.input_hash) && self.path.is_dir() {
            return Ok(SideEffectsCacheRestore::AlreadyApplied);
        }

        if !self.path.is_dir() {
            return Ok(SideEffectsCacheRestore::Miss);
        }

        copy_dir(&self.path, package_dir)?;
        Ok(SideEffectsCacheRestore::Restored)
    }

    pub(super) fn save(&self, package_dir: &Path) -> Result<()> {
        if self.path.is_dir() {
            write_marker(package_dir, &self.input_hash)?;
            return Ok(());
        }

        let parent = self.path.parent().ok_or_else(|| SnpmError::Internal {
            reason: format!(
                "side-effects cache path has no parent: {}",
                self.path.display()
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;

        write_marker(package_dir, &self.input_hash)?;

        let tmp_dir = parent.join(format!(
            "{SIDE_EFFECTS_TMP_PREFIX}{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));

        if tmp_dir.exists() {
            remove_path(&tmp_dir)?;
        }

        copy_dir(package_dir, &tmp_dir)?;

        match fs::rename(&tmp_dir, &self.path) {
            Ok(()) => Ok(()),
            Err(_source) if self.path.is_dir() => {
                remove_path(&tmp_dir).ok();
                Ok(())
            }
            Err(source) => {
                remove_path(&tmp_dir).ok();
                Err(SnpmError::WriteFile {
                    path: self.path.clone(),
                    source,
                })
            }
        }
    }
}

fn marker_matches(package_dir: &Path, expected: &str) -> bool {
    read_marker(package_dir).is_some_and(|value| value == expected)
}

fn read_marker(package_dir: &Path) -> Option<String> {
    let marker = fs::read_to_string(package_dir.join(SIDE_EFFECTS_CACHE_MARKER)).ok()?;
    let marker = marker.trim();

    (marker.len() == 128 && marker.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| marker.to_ascii_lowercase())
}

fn write_marker(package_dir: &Path, input_hash: &str) -> Result<()> {
    let path = package_dir.join(SIDE_EFFECTS_CACHE_MARKER);
    fs::write(&path, input_hash).map_err(|source| SnpmError::WriteFile { path, source })
}

fn hash_dir(package_dir: &Path) -> Result<String> {
    let mut hasher = Sha512::new();
    hash_dir_inner(package_dir, package_dir, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

fn hash_dir_inner(base: &Path, current: &Path, hasher: &mut Sha512) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(SIDE_EFFECTS_CACHE_MARKER) {
            continue;
        }

        let relative = path
            .strip_prefix(base)
            .map_err(|source| SnpmError::Io {
                path: path.clone(),
                source: std::io::Error::other(source),
            })?
            .to_string_lossy()
            .replace('\\', "/");

        let metadata = fs::symlink_metadata(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        hasher.update(relative.as_bytes());

        if metadata.file_type().is_symlink() {
            hasher.update(b"\0symlink\0");
            let target = fs::read_link(&path).map_err(|source| SnpmError::ReadFile {
                path: path.clone(),
                source,
            })?;
            hasher.update(target.to_string_lossy().as_bytes());
            continue;
        }

        if metadata.is_dir() {
            hasher.update(b"\0dir\0");
            hash_dir_inner(base, &path, hasher)?;
            continue;
        }

        hasher.update(b"\0file\0");
        let bytes = fs::read(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;
        hasher.update(&bytes);
    }

    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    if destination.symlink_metadata().is_ok() {
        remove_path(destination)?;
    }

    fs::create_dir_all(destination).map_err(|source_err| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source: source_err,
    })?;

    copy_dir_inner(source, source, destination)
}

fn copy_dir_inner(base: &Path, current: &Path, destination_root: &Path) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(base).map_err(|source| SnpmError::Io {
            path: path.clone(),
            source: std::io::Error::other(source),
        })?;
        let destination = destination_root.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|source| SnpmError::ReadFile {
                path: path.clone(),
                source,
            })?;
            create_symlink_like(&path, &target, &destination)?;
            continue;
        }

        if metadata.is_dir() {
            fs::create_dir_all(&destination).map_err(|source| SnpmError::WriteFile {
                path: destination.clone(),
                source,
            })?;
            copy_dir_inner(base, &path, destination_root)?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        clone_or_copy_file(&path, &destination).map_err(|source| SnpmError::WriteFile {
            path: destination.clone(),
            source,
        })?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let result = if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };

    result.map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn create_symlink_like(source: &Path, target: &Path, destination: &Path) -> Result<()> {
    let target_metadata = fs::metadata(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let link_result = if target_metadata.is_dir() {
        symlink_dir_entry(target, destination)
    } else {
        symlink_file_entry(target, destination)
    };

    link_result.map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{SideEffectsCacheEntry, SideEffectsCacheRestore};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    #[test]
    fn saves_and_restores_side_effects() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let package_dir = dir.path().join("package");

        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("package.json"),
            r#"{"name":"esbuild","version":"1.0.0"}"#,
        )
        .unwrap();
        fs::write(package_dir.join("built.txt"), "built").unwrap();

        let entry = SideEffectsCacheEntry::new(&config, "esbuild", "1.0.0", &package_dir).unwrap();
        entry.save(&package_dir).unwrap();

        fs::remove_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("package.json"),
            r#"{"name":"esbuild","version":"1.0.0"}"#,
        )
        .unwrap();

        let restore = entry.restore_if_available(&package_dir).unwrap();

        assert!(matches!(restore, SideEffectsCacheRestore::Restored));
        assert_eq!(
            fs::read_to_string(package_dir.join("built.txt")).unwrap(),
            "built"
        );
    }
}
