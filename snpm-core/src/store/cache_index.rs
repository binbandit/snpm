use super::filesystem::sanitize_name;
use crate::resolve::PackageId;
use crate::{Result, SnpmConfig, SnpmError};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const STORE_RESIDENCY_INDEX_VERSION: u32 = 1;
static NEXT_TMP_WRITE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreResidencyIndex {
    version: u32,
    packages: BTreeMap<PackageId, StoreResidencyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreResidencyEntry {
    root_relative_path: Option<PathBuf>,
}

pub(crate) struct StoreResidencyIndexView {
    packages: BTreeMap<PackageId, StoreResidencyEntry>,
}

impl Default for StoreResidencyIndex {
    fn default() -> Self {
        Self {
            version: STORE_RESIDENCY_INDEX_VERSION,
            packages: BTreeMap::new(),
        }
    }
}

impl StoreResidencyIndexView {
    pub(crate) fn resolve_root(&self, packages_dir: &Path, id: &PackageId) -> Option<PathBuf> {
        let package_dir = package_dir(packages_dir, id);
        let entry = self.packages.get(id)?;

        match entry.root_relative_path.as_deref() {
            None => Some(package_dir),
            Some(relative) if relative.as_os_str().is_empty() => Some(package_dir),
            Some(relative) if is_valid_relative_path(relative) => Some(package_dir.join(relative)),
            Some(_) => None,
        }
    }
}

pub(crate) fn load_store_residency_index_lossy(
    config: &SnpmConfig,
) -> Option<StoreResidencyIndexView> {
    let path = config.store_residency_index_path();
    let bytes = fs::read(path).ok()?;
    let index = bincode::deserialize::<StoreResidencyIndex>(&bytes).ok()?;

    (index.version == STORE_RESIDENCY_INDEX_VERSION).then_some(StoreResidencyIndexView {
        packages: index.packages,
    })
}

pub(crate) fn persist_store_residency_index(
    config: &SnpmConfig,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    if store_paths.is_empty() {
        return Ok(());
    }

    let path = config.store_residency_index_path();
    let parent = path.parent().ok_or_else(|| SnpmError::Internal {
        reason: format!(
            "store residency index path has no parent: {}",
            path.display()
        ),
    })?;
    fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
        path: parent.to_path_buf(),
        source,
    })?;

    let mut index = load_store_residency_index_lossy(config)
        .map(|index| StoreResidencyIndex {
            version: STORE_RESIDENCY_INDEX_VERSION,
            packages: index.packages,
        })
        .unwrap_or_default();

    let packages_dir = config.packages_dir();
    for (id, root) in store_paths {
        if let Some(entry) = entry_for_root(&packages_dir, id, root) {
            index.packages.insert(id.clone(), entry);
        }
    }

    let data = bincode::serialize(&index).map_err(|source| SnpmError::SerializeJson {
        path: path.clone(),
        reason: source.to_string(),
    })?;

    let tmp_path = parent.join(format!(
        ".store-residency-v1-{}.{}.tmp",
        std::process::id(),
        NEXT_TMP_WRITE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp_path, data).map_err(|source| SnpmError::WriteFile {
        path: tmp_path.clone(),
        source,
    })?;

    match fs::rename(&tmp_path, &path) {
        Ok(()) => Ok(()),
        Err(_source) if path.is_file() => {
            fs::remove_file(&path).ok();
            match fs::rename(&tmp_path, &path) {
                Ok(()) => Ok(()),
                Err(source) => {
                    fs::remove_file(&tmp_path).ok();
                    Err(SnpmError::WriteFile { path, source })
                }
            }
        }
        Err(source) => {
            fs::remove_file(&tmp_path).ok();
            Err(SnpmError::WriteFile { path, source })
        }
    }
}

fn entry_for_root(packages_dir: &Path, id: &PackageId, root: &Path) -> Option<StoreResidencyEntry> {
    let package_dir = package_dir(packages_dir, id);
    let relative = root.strip_prefix(&package_dir).ok()?;

    let root_relative_path = if relative.as_os_str().is_empty() {
        None
    } else if is_valid_relative_path(relative) {
        Some(relative.to_path_buf())
    } else {
        return None;
    };

    Some(StoreResidencyEntry { root_relative_path })
}

fn package_dir(packages_dir: &Path, id: &PackageId) -> PathBuf {
    packages_dir.join(sanitize_name(&id.name)).join(&id.version)
}

fn is_valid_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && !path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{load_store_residency_index_lossy, persist_store_residency_index};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::resolve::PackageId;

    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
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
    fn store_residency_index_round_trips_nested_roots() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let id = PackageId {
            name: "@scope/pkg".to_string(),
            version: "1.0.0".to_string(),
        };
        let root = config
            .packages_dir()
            .join("@scope_pkg")
            .join("1.0.0")
            .join("package");

        persist_store_residency_index(&config, &BTreeMap::from([(id.clone(), root)])).unwrap();

        let index = load_store_residency_index_lossy(&config).unwrap();
        let resolved = index.resolve_root(&config.packages_dir(), &id).unwrap();
        assert_eq!(
            resolved,
            config.packages_dir().join("@scope_pkg/1.0.0/package")
        );
    }
}
