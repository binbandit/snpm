use crate::config::OfflineMode;
use crate::console;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig};
use std::fs;
use std::path::Path;

/// Load cached metadata with default freshness checks (Online mode).
pub fn load_metadata(config: &SnpmConfig, name: &str) -> Option<RegistryPackage> {
    load_metadata_with_offline(config, name, OfflineMode::Online)
}

/// Load cached metadata respecting offline mode.
/// - Online: Only return fresh cache
/// - PreferOffline: Return stale cache if available
/// - Offline: Return any cache (caller must handle missing case)
pub fn load_metadata_with_offline(
    config: &SnpmConfig,
    name: &str,
    offline_mode: OfflineMode,
) -> Option<RegistryPackage> {
    let sanitized = sanitize_package_name(name);
    let cache_path = config.metadata_dir().join(&sanitized).join("index.json");

    if !cache_path.exists() {
        return None;
    }

    if let Ok(data) = fs::read_to_string(&cache_path)
        && let Ok(package) = serde_json::from_str::<RegistryPackage>(&data)
    {
        let fresh = is_fresh(config, &cache_path);

        // In PreferOffline or Offline mode, accept stale cache
        if fresh
            || matches!(
                offline_mode,
                OfflineMode::PreferOffline | OfflineMode::Offline
            )
        {
            if console::is_logging_enabled() {
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

    match serde_json::to_string(package) {
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

pub fn load_cached_headers(config: &SnpmConfig, name: &str) -> Option<CachedHeaders> {
    let sanitized = sanitize_package_name(name);
    let headers_path = config.metadata_dir().join(&sanitized).join("headers.json");

    if let Ok(data) = fs::read_to_string(&headers_path)
        && let Ok(headers) = serde_json::from_str::<CachedHeaders>(&data)
    {
        return Some(headers);
    }

    None
}

pub fn save_cached_headers(config: &SnpmConfig, name: &str, headers: &CachedHeaders) {
    let sanitized = sanitize_package_name(name);
    let cache_dir = config.metadata_dir().join(&sanitized);

    if let Ok(json) = serde_json::to_string(headers) {
        let _ = fs::write(cache_dir.join("headers.json"), json);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedHeaders {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

fn sanitize_package_name(name: &str) -> String {
    name.replace('/', "__")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::registry::{RegistryDist, RegistryPackage, RegistryVersion};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: Some(7),
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
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    fn make_package() -> RegistryPackage {
        let mut versions = BTreeMap::new();
        versions.insert(
            "1.0.0".to_string(),
            RegistryVersion {
                version: "1.0.0".to_string(),
                dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::new(),
                peer_dependencies_meta: BTreeMap::new(),
                bundled_dependencies: None,
                bundle_dependencies: None,
                dist: RegistryDist {
                    tarball: "https://example.com/pkg.tgz".to_string(),
                    integrity: None,
                },
                os: vec![],
                cpu: vec![],
                bin: None,
            },
        );
        let mut dist_tags = BTreeMap::new();
        dist_tags.insert("latest".to_string(), "1.0.0".to_string());
        RegistryPackage {
            versions,
            time: BTreeMap::new(),
            dist_tags,
        }
    }

    #[test]
    fn sanitize_package_name_simple() {
        assert_eq!(sanitize_package_name("lodash"), "lodash");
    }

    #[test]
    fn sanitize_package_name_scoped() {
        assert_eq!(sanitize_package_name("@types/node"), "@types__node");
    }

    #[test]
    fn sanitize_package_name_multiple_slashes() {
        assert_eq!(sanitize_package_name("a/b/c"), "a__b__c");
    }

    #[test]
    fn save_and_load_metadata_roundtrip() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        let package = make_package();

        save_metadata(&config, "test-pkg", &package).unwrap();

        let loaded = load_metadata_with_offline(
            &config,
            "test-pkg",
            crate::config::OfflineMode::PreferOffline,
        );
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert!(loaded.versions.contains_key("1.0.0"));
        assert_eq!(
            loaded.dist_tags.get("latest").map(String::as_str),
            Some("1.0.0")
        );
    }

    #[test]
    fn load_metadata_returns_none_when_not_cached() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());

        let loaded = load_metadata(&config, "nonexistent-pkg");
        assert!(loaded.is_none());
    }

    #[test]
    fn save_and_load_cached_headers_roundtrip() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());

        // Create the metadata dir first
        std::fs::create_dir_all(config.metadata_dir().join("test-pkg")).unwrap();

        let headers = CachedHeaders {
            etag: Some("\"abc123\"".to_string()),
            last_modified: Some("Thu, 01 Jan 2026 00:00:00 GMT".to_string()),
        };

        save_cached_headers(&config, "test-pkg", &headers);

        let loaded = load_cached_headers(&config, "test-pkg");
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(
            loaded.last_modified.as_deref(),
            Some("Thu, 01 Jan 2026 00:00:00 GMT")
        );
    }

    #[test]
    fn load_cached_headers_returns_none_when_not_cached() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());

        assert!(load_cached_headers(&config, "nonexistent").is_none());
    }

    #[test]
    fn is_fresh_returns_true_for_recent_file() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        let file = dir.path().join("recent");
        std::fs::write(&file, "data").unwrap();

        assert!(is_fresh(&config, &file));
    }

    #[test]
    fn is_fresh_returns_false_when_no_cache_age_configured() {
        let dir = tempdir().unwrap();
        let mut config = make_config(dir.path().to_path_buf());
        config.min_package_cache_age_days = None;
        let file = dir.path().join("file");
        std::fs::write(&file, "data").unwrap();

        assert!(!is_fresh(&config, &file));
    }

    #[test]
    fn is_fresh_returns_false_for_nonexistent_file() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        assert!(!is_fresh(&config, &dir.path().join("nonexistent")));
    }
}
