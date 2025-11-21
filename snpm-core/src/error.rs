use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SnpmError {
    #[error("Failed to read file {path:?}: {source}")]
    ReadFile { path: PathBuf, source: std::io::Error },

    #[error("Failed to parse JSON in {path:?}: {source}")]
    ParseJson { path: PathBuf, source: serde_json::Error },

    #[error("Project manifest package.json not found at {path:?}")]
    ManifestMissing { path: PathBuf },

    #[error("Invalid manifest in {path:?}: {reason}")]
    ManifestInvalid { path: PathBuf, reason: String },
}
