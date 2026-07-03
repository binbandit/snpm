use super::metadata::storage::{read_cached_package_record, write_cached_headers};
use crate::SnpmConfig;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedHeaders {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub fn load_cached_headers(config: &SnpmConfig, name: &str) -> Option<CachedHeaders> {
    read_cached_package_record(config, name).and_then(|record| record.headers)
}

pub fn save_cached_headers(config: &SnpmConfig, name: &str, headers: &CachedHeaders) {
    let _ = write_cached_headers(config, name, headers);
}

#[cfg(test)]
mod tests {
    use super::{CachedHeaders, load_cached_headers, save_cached_headers};
    use crate::config::SnpmConfig;

    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            ..SnpmConfig::for_tests()
        }
    }

    #[test]
    fn save_and_load_cached_headers_roundtrip() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());

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
