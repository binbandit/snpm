mod freshness;
mod storage;

use super::paths::{metadata_cache_path, package_cache_dir};
use crate::config::OfflineMode;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig};

use freshness::{is_fresh, log_cache_hit};
use storage::{log_stale_cache, read_cached_package, write_cached_package};

pub fn load_metadata(config: &SnpmConfig, name: &str) -> Option<RegistryPackage> {
    load_metadata_with_offline(config, name, OfflineMode::Online)
}

pub fn load_metadata_with_offline(
    config: &SnpmConfig,
    name: &str,
    offline_mode: OfflineMode,
) -> Option<RegistryPackage> {
    let cache_path = metadata_cache_path(config, name);
    if !cache_path.exists() {
        return None;
    }

    if let Some(package) = read_cached_package(&cache_path) {
        let fresh = is_fresh(config, &cache_path);
        if fresh
            || matches!(
                offline_mode,
                OfflineMode::PreferOffline | OfflineMode::Offline
            )
        {
            log_cache_hit(name, &cache_path, fresh);
            return Some(package);
        }

        log_stale_cache(name);
    }

    None
}

pub fn save_metadata(config: &SnpmConfig, name: &str, package: &RegistryPackage) -> Result<()> {
    let cache_dir = package_cache_dir(config, name);
    let cache_path = metadata_cache_path(config, name);
    write_cached_package(&cache_dir, &cache_path, name, package)
}

#[cfg(test)]
mod tests;
