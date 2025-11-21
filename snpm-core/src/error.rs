use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SnpmError {
    #[error("Failed to read file {path:?}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to write file {path:?}: {source}")]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to parse JSON in {path:?}: {source}")]
    ParseJson {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("Failed to serialize JSON for {path:?}: {source}")]
    SerializeJson {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("HTTP request to {url} failed: {source}")]
    Http { url: String, source: reqwest::Error },

    #[error("Invalid semver value {value}: {source}")]
    Semver {
        value: String,
        source: semver::Error,
    },

    #[error("Failed to resolve package {name}@{range}")]
    ResolutionFailed { name: String, range: String },

    #[error("Failed to write lockfile at {path:?}: {source}")]
    LockfileWrite {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("Package not found in store: {name}@{version}")]
    StoreMissing { name: String, version: String },

    #[error("Failed to unpack archive into {path:?}: {source}")]
    Archive {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Project manifest package.json not found at {path:?}")]
    ManifestMissing { path: PathBuf },

    #[error("Invalid manifest in {path:?}: {reason}")]
    ManifestInvalid { path: PathBuf, reason: String },
}
