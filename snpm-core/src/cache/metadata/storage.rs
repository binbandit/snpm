use crate::Result;
use crate::SnpmConfig;
use crate::console;
use crate::registry::RegistryPackage;

use super::super::headers::CachedHeaders;
use super::super::paths::{headers_cache_path, metadata_cache_path, metadata_shard_path};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const PACKUMENT_CACHE_VERSION: u32 = 1;

static SHARD_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone)]
pub(in crate::cache) struct CachedPackageRecord {
    pub(in crate::cache) package: Option<RegistryPackage>,
    pub(in crate::cache) headers: Option<CachedHeaders>,
    pub(in crate::cache) updated_at_unix_secs: u64,
    pub(in crate::cache) cache_path: PathBuf,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct CachedPackageEntry {
    #[serde(default)]
    package: Option<RegistryPackage>,
    #[serde(default)]
    headers: Option<CachedHeaders>,
    #[serde(default)]
    updated_at_unix_secs: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MetadataShard {
    version: u32,
    packages: BTreeMap<String, CachedPackageEntry>,
}

impl Default for MetadataShard {
    fn default() -> Self {
        Self {
            version: PACKUMENT_CACHE_VERSION,
            packages: BTreeMap::new(),
        }
    }
}

pub(in crate::cache) fn read_cached_package_record(
    config: &SnpmConfig,
    name: &str,
) -> Option<CachedPackageRecord> {
    read_shard_record(config, name).or_else(|| read_legacy_record(config, name))
}

pub(in crate::cache) fn write_cached_package(
    config: &SnpmConfig,
    name: &str,
    package: &RegistryPackage,
) -> Result<()> {
    write_record_update(config, name, |entry| {
        entry.package = Some(package.clone());
    })
}

pub(in crate::cache) fn write_cached_headers(
    config: &SnpmConfig,
    name: &str,
    headers: &CachedHeaders,
) -> Result<()> {
    write_record_update(config, name, |entry| {
        entry.headers = Some(headers.clone());
    })
}

pub(in crate::cache) fn write_cached_package_record(
    config: &SnpmConfig,
    name: &str,
    package: &RegistryPackage,
    headers: Option<&CachedHeaders>,
) -> Result<()> {
    write_record_update(config, name, |entry| {
        entry.package = Some(package.clone());
        if let Some(headers) = headers {
            entry.headers = Some(headers.clone());
        }
    })
}

fn read_shard_record(config: &SnpmConfig, name: &str) -> Option<CachedPackageRecord> {
    let cache_path = metadata_shard_path(config, name);
    let shard = read_shard(&cache_path)?;
    let entry = shard.packages.get(name)?.clone();
    if entry.package.is_none() && entry.headers.is_none() {
        return None;
    }

    Some(CachedPackageRecord {
        package: entry.package,
        headers: entry.headers,
        updated_at_unix_secs: entry.updated_at_unix_secs,
        cache_path,
    })
}

fn read_legacy_record(config: &SnpmConfig, name: &str) -> Option<CachedPackageRecord> {
    let package_path = metadata_cache_path(config, name);
    let headers_path = headers_cache_path(config, name);
    let package = read_legacy_package(&package_path);
    let headers = read_legacy_headers(&headers_path);

    if package.is_none() && headers.is_none() {
        return None;
    }

    let cache_path = if package.is_some() {
        package_path.clone()
    } else {
        headers_path.clone()
    };

    Some(CachedPackageRecord {
        package,
        headers,
        updated_at_unix_secs: latest_modified_unix_secs(&[&package_path, &headers_path]),
        cache_path,
    })
}

fn write_record_update<F>(config: &SnpmConfig, name: &str, update: F) -> Result<()>
where
    F: FnOnce(&mut CachedPackageEntry),
{
    let cache_path = metadata_shard_path(config, name);
    if let Some(parent) = cache_path.parent()
        && let Err(error) = fs::create_dir_all(parent)
    {
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "failed to create metadata cache dir {}: {}",
                parent.display(),
                error
            ));
        }
        return Ok(());
    }

    let _guard = shard_write_lock().lock().ok();
    let mut shard = read_shard(&cache_path).unwrap_or_default();
    let entry = shard.packages.entry(name.to_string()).or_default();
    update(entry);
    entry.updated_at_unix_secs = now_unix_secs();

    match bincode::serialize(&shard) {
        Ok(bytes) => {
            if let Err(error) = write_atomic(&cache_path, &bytes) {
                if console::is_logging_enabled() {
                    console::verbose(&format!(
                        "failed to write metadata cache for {}: {}",
                        name, error
                    ));
                }
            } else if console::is_logging_enabled() {
                console::verbose(&format!(
                    "saved metadata cache for {} to {}",
                    name,
                    cache_path.display()
                ));
            }
        }
        Err(error) => {
            if console::is_logging_enabled() {
                console::verbose(&format!(
                    "failed to serialize metadata for {}: {}",
                    name, error
                ));
            }
        }
    }

    Ok(())
}

fn read_shard(path: &Path) -> Option<MetadataShard> {
    let bytes = fs::read(path).ok()?;
    let shard = bincode::deserialize::<MetadataShard>(&bytes).ok()?;
    (shard.version == PACKUMENT_CACHE_VERSION).then_some(shard)
}

fn read_legacy_package(cache_path: &Path) -> Option<RegistryPackage> {
    if let Ok(data) = fs::read_to_string(cache_path)
        && let Ok(package) = serde_json::from_str::<RegistryPackage>(&data)
    {
        return Some(package);
    }

    None
}

fn read_legacy_headers(cache_path: &Path) -> Option<CachedHeaders> {
    if let Ok(data) = fs::read_to_string(cache_path)
        && let Ok(headers) = serde_json::from_str::<CachedHeaders>(&data)
    {
        return Some(headers);
    }

    None
}

fn latest_modified_unix_secs(paths: &[&Path]) -> u64 {
    paths
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .filter_map(|metadata| metadata.modified().ok())
        .filter_map(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .max()
        .unwrap_or_default()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cache");
    let tmp_path = path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ));

    fs::write(&tmp_path, bytes)?;
    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(_source) if path.is_file() => {
            fs::remove_file(path).ok();
            match fs::rename(&tmp_path, path) {
                Ok(()) => Ok(()),
                Err(source) => {
                    fs::remove_file(&tmp_path).ok();
                    Err(source)
                }
            }
        }
        Err(source) => {
            fs::remove_file(&tmp_path).ok();
            Err(source)
        }
    }
}

fn shard_write_lock() -> &'static Mutex<()> {
    SHARD_WRITE_LOCK.get_or_init(|| Mutex::new(()))
}

pub(in crate::cache) fn log_stale_cache(name: &str) {
    if console::is_logging_enabled() {
        console::verbose(&format!(
            "cached metadata for {} is stale, will refetch",
            name
        ));
    }
}
