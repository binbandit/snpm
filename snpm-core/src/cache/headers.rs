use super::paths::{headers_cache_path, package_cache_dir};
use crate::SnpmConfig;

use std::fs;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedHeaders {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub fn load_cached_headers(config: &SnpmConfig, name: &str) -> Option<CachedHeaders> {
    let headers_path = headers_cache_path(config, name);

    if let Ok(data) = fs::read_to_string(&headers_path)
        && let Ok(headers) = serde_json::from_str::<CachedHeaders>(&data)
    {
        return Some(headers);
    }

    None
}

pub fn save_cached_headers(config: &SnpmConfig, name: &str, headers: &CachedHeaders) {
    let cache_dir = package_cache_dir(config, name);
    let headers_path = headers_cache_path(config, name);

    let _ = fs::create_dir_all(&cache_dir);

    if let Ok(json) = serde_json::to_string(headers) {
        let _ = fs::write(headers_path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::{CachedHeaders, load_cached_headers, save_cached_headers};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

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

    #[test]
    fn save_and_load_cached_headers_roundtrip() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());

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
}
