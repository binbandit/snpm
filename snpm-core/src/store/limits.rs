use std::sync::{Arc, OnceLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::resolve::PackageId;
use crate::{Result, SnpmConfig, SnpmError};

const DEFAULT_DOWNLOAD_CONCURRENCY: usize = 32;
const MAX_REGISTRY_CONCURRENCY: usize = 64;
const MAX_STORE_TASK_CONCURRENCY: usize = 48;
const MIN_EXTRACTION_CONCURRENCY: usize = 4;
const MAX_EXTRACTION_CONCURRENCY: usize = 32;
const OPEN_FILE_DESCRIPTOR_RESERVE: usize = 96;
const REGISTRY_DESCRIPTORS_PER_TASK: usize = 4;
const DOWNLOAD_DESCRIPTORS_PER_TASK: usize = 8;
const STORE_DESCRIPTORS_PER_TASK: usize = 4;
const EXTRACTION_DESCRIPTORS_PER_TASK: usize = 4;

/// Limits concurrent tarball downloads to prevent bandwidth saturation and CDN
/// throttling. Downloads that finish release their permit immediately so
/// extraction (governed by a separate semaphore) can overlap with the next
/// batch of downloads.
pub(super) fn download_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(download_concurrency()))
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

pub(crate) fn registry_task_concurrency(config: &SnpmConfig) -> usize {
    descriptor_limited_concurrency(
        config.registry_concurrency,
        1,
        MAX_REGISTRY_CONCURRENCY,
        open_file_descriptor_limit(),
        REGISTRY_DESCRIPTORS_PER_TASK,
    )
}

pub(crate) fn store_task_concurrency(config: &SnpmConfig) -> usize {
    descriptor_limited_concurrency(
        config.registry_concurrency,
        1,
        MAX_STORE_TASK_CONCURRENCY,
        open_file_descriptor_limit(),
        STORE_DESCRIPTORS_PER_TASK,
    )
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

pub(super) fn download_concurrency() -> usize {
    descriptor_limited_concurrency(
        DEFAULT_DOWNLOAD_CONCURRENCY,
        1,
        DEFAULT_DOWNLOAD_CONCURRENCY,
        open_file_descriptor_limit(),
        DOWNLOAD_DESCRIPTORS_PER_TASK,
    )
}

fn extraction_concurrency_limit(cpu_count: usize) -> usize {
    let cpu_limited = cpu_count.clamp(MIN_EXTRACTION_CONCURRENCY, MAX_EXTRACTION_CONCURRENCY);

    descriptor_limited_concurrency(
        cpu_limited,
        MIN_EXTRACTION_CONCURRENCY,
        MAX_EXTRACTION_CONCURRENCY,
        open_file_descriptor_limit(),
        EXTRACTION_DESCRIPTORS_PER_TASK,
    )
}

fn descriptor_limited_concurrency(
    requested: usize,
    minimum: usize,
    maximum: usize,
    open_file_limit: Option<usize>,
    descriptors_per_task: usize,
) -> usize {
    let requested = requested.max(minimum).min(maximum);
    let Some(open_file_limit) = open_file_limit else {
        return requested;
    };

    let descriptor_budget = open_file_limit.saturating_sub(OPEN_FILE_DESCRIPTOR_RESERVE);
    let descriptor_limit = (descriptor_budget / descriptors_per_task.max(1)).max(minimum);

    requested.min(descriptor_limit)
}

fn open_file_descriptor_limit() -> Option<usize> {
    static LIMIT: OnceLock<Option<usize>> = OnceLock::new();
    *LIMIT.get_or_init(read_open_file_descriptor_limit)
}

#[cfg(unix)]
fn read_open_file_descriptor_limit() -> Option<usize> {
    let mut limit = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: getrlimit initializes the rlimit struct when it returns success.
    let result = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) };
    if result != 0 {
        return None;
    }

    let soft_limit = unsafe { limit.assume_init().rlim_cur };
    if soft_limit == libc::RLIM_INFINITY {
        return None;
    }

    usize::try_from(soft_limit).ok()
}

#[cfg(not(unix))]
fn read_open_file_descriptor_limit() -> Option<usize> {
    None
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_DOWNLOAD_CONCURRENCY, descriptor_limited_concurrency, extraction_concurrency_limit,
        registry_task_concurrency, store_task_concurrency,
    };
    use crate::config::SnpmConfig;
    
    

    fn test_config(registry_concurrency: usize) -> SnpmConfig {
    SnpmConfig {
        registry_concurrency,
        ..SnpmConfig::for_tests()
    }
}

    #[test]
    fn extraction_limit_keeps_small_machines_busy_without_oversubscribing() {
        assert_eq!(extraction_concurrency_limit(0), 4);
        assert_eq!(extraction_concurrency_limit(1), 4);
        assert_eq!(extraction_concurrency_limit(2), 4);
        assert_eq!(extraction_concurrency_limit(4), 4);
    }

    #[test]
    fn extraction_limit_tracks_cpu_count_until_cap() {
        assert_eq!(extraction_concurrency_limit(8), 8);
        assert_eq!(extraction_concurrency_limit(16), 16);
        assert_eq!(extraction_concurrency_limit(32), 32);
        assert_eq!(extraction_concurrency_limit(96), 32);
    }

    #[test]
    fn registry_task_concurrency_is_capped_separately_from_config() {
        assert_eq!(registry_task_concurrency(&test_config(0)), 1);
        assert!((1..=64).contains(&registry_task_concurrency(&test_config(128))));
    }

    #[test]
    fn store_task_concurrency_is_capped_separately_from_registry_fetches() {
        assert_eq!(store_task_concurrency(&test_config(0)), 1);
        assert!((1..=48).contains(&store_task_concurrency(&test_config(128))));
    }

    #[test]
    fn descriptor_budget_reduces_concurrency_under_low_open_file_limits() {
        assert_eq!(
            descriptor_limited_concurrency(
                DEFAULT_DOWNLOAD_CONCURRENCY,
                1,
                DEFAULT_DOWNLOAD_CONCURRENCY,
                Some(256),
                8,
            ),
            20
        );
        assert_eq!(descriptor_limited_concurrency(128, 1, 64, Some(256), 4), 40);
    }

    #[test]
    fn descriptor_budget_keeps_minimum_for_tiny_limits() {
        assert_eq!(descriptor_limited_concurrency(128, 1, 64, Some(96), 4), 1);
        assert_eq!(descriptor_limited_concurrency(8, 2, 8, Some(96), 4), 2);
    }
}
