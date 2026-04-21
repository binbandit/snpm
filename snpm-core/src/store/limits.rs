use std::sync::OnceLock;

use tokio::sync::Semaphore;

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

fn extraction_concurrency_limit(cpu_count: usize) -> usize {
    cpu_count
        .max(MIN_EXTRACTION_CONCURRENCY)
        .min(MAX_EXTRACTION_CONCURRENCY)
}

#[cfg(test)]
mod tests {
    use super::extraction_concurrency_limit;

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
}
