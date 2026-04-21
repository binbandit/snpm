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

    let age_days = now.saturating_sub(std::time::Duration::from_secs(updated_at_unix_secs));
    age_days.as_secs() / 86400 < max_age_days as u64
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
