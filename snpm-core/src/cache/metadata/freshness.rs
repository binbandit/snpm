use crate::SnpmConfig;
use crate::console;

use std::fs;
use std::path::Path;

pub(super) fn is_fresh(config: &SnpmConfig, cache_path: &Path) -> bool {
    let Some(max_age_days) = config.min_package_cache_age_days else {
        return false;
    };

    if let Ok(metadata) = fs::metadata(cache_path)
        && let Ok(modified) = metadata.modified()
        && let Ok(elapsed) = modified.elapsed()
    {
        let age_days = elapsed.as_secs() / 86400;
        return age_days < max_age_days as u64;
    }

    false
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
