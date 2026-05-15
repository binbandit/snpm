mod freshness;
pub(in crate::cache) mod storage;

use super::headers::CachedHeaders;
use crate::config::OfflineMode;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig};

use freshness::{is_fresh, log_cache_hit};
use storage::{
    log_stale_cache, read_cached_package_record, write_cached_package, write_cached_package_record,
};

pub fn load_metadata(config: &SnpmConfig, name: &str) -> Option<RegistryPackage> {
    load_metadata_with_offline(config, name, OfflineMode::Online)
}

pub fn load_metadata_with_offline(
    config: &SnpmConfig,
    name: &str,
    offline_mode: OfflineMode,
) -> Option<RegistryPackage> {
    if let Some(record) = read_cached_package_record(config, name)
        && let Some(package) = record.package
    {
        let fresh = is_fresh(config, record.updated_at_unix_secs);
        let has_required_age_metadata =
            config.min_package_age_days.is_none() || !package.time.is_empty();
        let usable_for_mode = fresh
            || matches!(
                offline_mode,
                OfflineMode::PreferOffline | OfflineMode::Offline
            );
        if usable_for_mode && has_required_age_metadata {
            log_cache_hit(name, &record.cache_path, fresh);
            return Some(package);
        }

        log_stale_cache(name);
    }

    None
}

pub fn save_metadata(config: &SnpmConfig, name: &str, package: &RegistryPackage) -> Result<()> {
    write_cached_package(config, name, package)
}

pub fn save_metadata_with_headers(
    config: &SnpmConfig,
    name: &str,
    package: &RegistryPackage,
    headers: Option<&CachedHeaders>,
) -> Result<()> {
    write_cached_package_record(config, name, package, headers)
}

#[cfg(test)]
mod tests;
