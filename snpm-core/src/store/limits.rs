use std::sync::{Arc, OnceLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::resolve::PackageId;
use crate::{Result, SnpmConfig, SnpmError};

const MIN_EXTRACTION_CONCURRENCY: usize = 2;
const MAX_EXTRACTION_CONCURRENCY: usize = 16;

/// Limits concurrent tarball downloads to prevent bandwidth saturation and CDN
/// throttling. Downloads that finish release their permit immediately so
/// extraction (governed by a separate semaphore) can overlap with the next
/// batch of downloads.
pub(super) fn download_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(64))
}

/// Limits concurrent disk-bound operations (dir prep + tarball extraction) to
/// prevent I/O thrashing and excessive blocking-thread-pool growth.
pub(super) fn extraction_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| {
        let cpus = std::thread::available_parallelism()
            .map(|parallelism| parallelism.get())
            .unwrap_or(8);
        Semaphore::new(extraction_concurrency_limit(cpus))
    })
}

pub(crate) fn store_task_concurrency(config: &SnpmConfig) -> usize {
    config.registry_concurrency.max(1)
}

pub(crate) async fn acquire_store_task_permit(
    semaphore: Arc<Semaphore>,
    id: &PackageId,
) -> Result<OwnedSemaphorePermit> {
    semaphore
        .acquire_owned()
        .await
        .map_err(|error| SnpmError::Internal {
            reason: format!(
                "store task semaphore closed while materializing {}@{}: {error}",
                id.name, id.version
            ),
        })
}

fn extraction_concurrency_limit(cpu_count: usize) -> usize {
    cpu_count
        .max(MIN_EXTRACTION_CONCURRENCY)
        .min(MAX_EXTRACTION_CONCURRENCY)
}

#[cfg(test)]
mod tests {
    use super::{extraction_concurrency_limit, store_task_concurrency};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn test_config(registry_concurrency: usize) -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
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
            registry_concurrency,
            verbose: false,
            log_file: None,
        }
    }

    #[test]
    fn extraction_limit_keeps_small_machines_busy_without_oversubscribing() {
        assert_eq!(extraction_concurrency_limit(0), 2);
        assert_eq!(extraction_concurrency_limit(1), 2);
        assert_eq!(extraction_concurrency_limit(2), 2);
        assert_eq!(extraction_concurrency_limit(4), 4);
    }

    #[test]
    fn extraction_limit_tracks_cpu_count_until_cap() {
        assert_eq!(extraction_concurrency_limit(8), 8);
        assert_eq!(extraction_concurrency_limit(16), 16);
        assert_eq!(extraction_concurrency_limit(32), 16);
        assert_eq!(extraction_concurrency_limit(96), 16);
    }

    #[test]
    fn store_task_concurrency_never_drops_below_one() {
        assert_eq!(store_task_concurrency(&test_config(0)), 1);
        assert_eq!(store_task_concurrency(&test_config(128)), 128);
    }
}
