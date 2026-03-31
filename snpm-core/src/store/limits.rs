use std::sync::OnceLock;

use tokio::sync::Semaphore;

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
        Semaphore::new((cpus * 4).clamp(16, 256))
    })
}
