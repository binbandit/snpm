use crate::console;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig};
use std::fs;
use std::path::Path;

pub fn load_metadata(config: &SnpmConfig, name: &str) -> Option<RegistryPackage> {
    let sanitized = sanitize_package_name(name);
    let cache_path = config.metadata_dir().join(&sanitized).join("index.json");

    if !cache_path.exists() {
        return None;
    }

    if let Ok(data) = fs::read_to_string(&cache_path)
        && let Ok(package) = serde_json::from_str::<RegistryPackage>(&data)
    {
        if is_fresh(config, &cache_path) {
            if console::is_logging_enabled() {
                console::verbose(&format!(
                    "using cached metadata for {} from {}",
                    name,
                    cache_path.display()
                ));
            }
            return Some(package);
        } else if console::is_logging_enabled() {
            console::verbose(&format!(
                "cached metadata for {} is stale, will refetch",
                name
            ));
        }
    }

    None
}

pub fn save_metadata(config: &SnpmConfig, name: &str, package: &RegistryPackage) -> Result<()> {
    let sanitized = sanitize_package_name(name);
    let cache_dir = config.metadata_dir().join(&sanitized);
    let cache_path = cache_dir.join("index.json");

    if let Err(e) = fs::create_dir_all(&cache_dir) {
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "failed to create metadata cache dir {}: {}",
                cache_dir.display(),
                e
            ));
        }
        return Ok(());
    }

    match serde_json::to_string_pretty(package) {
        Ok(json) => {
            if let Err(e) = fs::write(&cache_path, json) {
                if console::is_logging_enabled() {
                    console::verbose(&format!(
                        "failed to write metadata cache for {}: {}",
                        name, e
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
        Err(e) => {
            if console::is_logging_enabled() {
                console::verbose(&format!("failed to serialize metadata for {}: {}", name, e));
            }
        }
    }

    Ok(())
}

fn is_fresh(config: &SnpmConfig, cache_path: &Path) -> bool {
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

fn sanitize_package_name(name: &str) -> String {
    name.replace('/', "__")
}
