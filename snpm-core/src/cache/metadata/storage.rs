use crate::Result;
use crate::console;
use crate::registry::RegistryPackage;

use std::fs;
use std::path::Path;

pub(super) fn read_cached_package(cache_path: &Path) -> Option<RegistryPackage> {
    if let Ok(data) = fs::read_to_string(cache_path)
        && let Ok(package) = serde_json::from_str::<RegistryPackage>(&data)
    {
        return Some(package);
    }

    None
}

pub(super) fn write_cached_package(
    cache_dir: &Path,
    cache_path: &Path,
    name: &str,
    package: &RegistryPackage,
) -> Result<()> {
    if let Err(error) = fs::create_dir_all(cache_dir) {
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "failed to create metadata cache dir {}: {}",
                cache_dir.display(),
                error
            ));
        }
        return Ok(());
    }

    match serde_json::to_string(package) {
        Ok(json) => {
            if let Err(error) = fs::write(cache_path, json) {
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

pub(super) fn log_stale_cache(name: &str) {
    if console::is_logging_enabled() {
        console::verbose(&format!(
            "cached metadata for {} is stale, will refetch",
            name
        ));
    }
}
