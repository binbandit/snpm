use crate::SnpmConfig;
use crate::console;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn is_fresh(config: &SnpmConfig, updated_at_unix_secs: u64) -> bool {
    let Some(max_age_days) = config.min_package_cache_age_days else {
        return false;
    };

    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };

    // A timestamp from the future (clock rollback, restored backup, NFS
    // skew) would otherwise saturate to age 0 and read as fresh for the
    // whole window; treat it as stale instead.
    let now_secs = now.as_secs();
    if updated_at_unix_secs > now_secs {
        return false;
    }

    (now_secs - updated_at_unix_secs) / 86400 < max_age_days as u64
}

pub(super) fn log_cache_hit(name: &str, cache_path: &Path, fresh: bool) {
    if !console::is_logging_enabled() {
        return;
    }

    let status = if fresh {
        "fresh"
    } else {
        "stale (offline mode)"
    };
    console::verbose(&format!(
        "using {} cached metadata for {} from {}",
        status,
        name,
        cache_path.display()
    ));
}
