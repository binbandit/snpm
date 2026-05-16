use crate::{Result, SnpmConfig, SnpmError, http};

use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{Duration, SystemTime};

const DIST_INDEX_PATH: &str = "/index.json";
const DEFAULT_DISTRO_URL: &str = "https://nodejs.org/dist";
const INDEX_TTL: Duration = Duration::from_secs(60 * 60 * 6);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeRelease {
    pub version: String,
    pub date: String,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub npm: Option<String>,
    #[serde(default)]
    pub lts: LtsField,
    #[serde(default)]
    pub security: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LtsField {
    None(bool),
    Codename(String),
}

impl Default for LtsField {
    fn default() -> Self {
        LtsField::None(false)
    }
}

impl LtsField {
    pub fn codename(&self) -> Option<&str> {
        match self {
            LtsField::Codename(name) => Some(name.as_str()),
            LtsField::None(_) => None,
        }
    }
}

pub fn distro_url() -> String {
    std::env::var("SNPM_NODE_DISTRO_URL").unwrap_or_else(|_| DEFAULT_DISTRO_URL.to_string())
}

pub async fn fetch_index(config: &SnpmConfig, force_refresh: bool) -> Result<Vec<NodeRelease>> {
    let cache_path = config.node_index_cache_path();

    if !force_refresh && cache_is_fresh(&cache_path) {
        if let Some(releases) = read_cached(&cache_path) {
            return Ok(releases);
        }
    }

    let url = format!("{}{}", distro_url(), DIST_INDEX_PATH);
    let client = http::create_client()?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?;

    if !response.status().is_success() {
        return Err(SnpmError::Tarball {
            url,
            reason: format!("HTTP {}", response.status()),
        });
    }

    let bytes = response.bytes().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let releases: Vec<NodeRelease> =
        serde_json::from_slice(&bytes).map_err(|source| SnpmError::ParseJson {
            path: cache_path.clone(),
            source,
        })?;

    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(&cache_path, &bytes).map_err(|source| SnpmError::WriteFile {
        path: cache_path,
        source,
    })?;

    Ok(releases)
}

pub fn read_cached_index(config: &SnpmConfig) -> Option<Vec<NodeRelease>> {
    read_cached(&config.node_index_cache_path())
}

fn cache_is_fresh(cache_path: &std::path::Path) -> bool {
    let Ok(metadata) = fs::metadata(cache_path) else {
        return false;
    };

    let Ok(modified) = metadata.modified() else {
        return false;
    };

    SystemTime::now()
        .duration_since(modified)
        .map(|age| age <= INDEX_TTL)
        .unwrap_or(false)
}

fn read_cached(cache_path: &std::path::Path) -> Option<Vec<NodeRelease>> {
    let bytes = fs::read(cache_path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::{LtsField, NodeRelease};

    #[test]
    fn deserializes_lts_string_codename() {
        let json = r#"{"version":"v20.10.0","date":"2024-01-15","files":[],"lts":"Iron"}"#;
        let release: NodeRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.lts.codename(), Some("Iron"));
    }

    #[test]
    fn deserializes_non_lts_release() {
        let json = r#"{"version":"v21.0.0","date":"2024-01-15","files":[],"lts":false}"#;
        let release: NodeRelease = serde_json::from_str(json).unwrap();
        assert!(matches!(release.lts, LtsField::None(false)));
    }
}
