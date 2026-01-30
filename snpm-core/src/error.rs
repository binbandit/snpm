use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
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

    #[error("Failed to serialize JSON for {path:?}: {reason}")]
    SerializeJson { path: PathBuf, reason: String },

    #[error("Failed to create HTTP client: {source}")]
    HttpClient { source: reqwest::Error },

    #[error("HTTP error when requesting {url}: {source}")]
    Http { url: String, source: reqwest::Error },

    #[error("Failed to decode registry response for {name}: {source}")]
    RegistryDecode {
        name: String,
        source: serde_json::Error,
    },

    #[error("Unable to resolve {name}@{range}: {reason}")]
    ResolutionFailed {
        name: String,
        range: String,
        reason: String,
    },

    #[error("Project manifest package.json not found at {path:?}")]
    ManifestMissing { path: PathBuf },

    #[error("Invalid manifest in {path:?}: {reason}")]
    ManifestInvalid { path: PathBuf, reason: String },

    #[error("Lockfile error at {path:?}: {reason}")]
    Lockfile { path: PathBuf, reason: String },

    #[error("Workspace config error at {path:?}: {reason}")]
    WorkspaceConfig { path: PathBuf, reason: String },

    #[error("Archive error at {path:?}: {source}")]
    Archive {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to download tarball from {url}: {reason}")]
    Tarball { url: String, reason: String },

    #[error("Failed to write lockfile to {path:?}: {source}")]
    LockfileWrite {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("Invalid semver {value}: {reason}")]
    Semver { value: String, reason: String },

    #[error("Package {name}@{version} missing from store")]
    StoreMissing { name: String, version: String },

    #[error("Package {name}@{version} missing from resolution graph")]
    GraphMissing { name: String, version: String },

    #[error("Script {name} not found in package.json")]
    ScriptMissing { name: String },

    #[error("Script {name} failed with exit code {code}")]
    ScriptFailed { name: String, code: i32 },

    #[error("Failed to run script {name}: {reason}")]
    ScriptRun { name: String, reason: String },

    #[error("Internal task failed: {reason}")]
    TaskJoin { reason: String },

    #[error("Internal store task failed: {reason}")]
    StoreTask { reason: String },

    #[error("Authentication error: {reason}")]
    Auth { reason: String },

    #[error("I/O error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Package {name}@{version} not found in project dependencies")]
    PackageNotInstalled { name: String, version: String },

    #[error("Patch not found for {name}: {reason}")]
    PatchNotFound { name: String, reason: String },

    #[error("Failed to create patch for {name}: {reason}")]
    PatchCreate { name: String, reason: String },

    #[error("Failed to apply patch for {name}: {reason}")]
    PatchApply { name: String, reason: String },

    #[error("No active patch session found at {path:?}")]
    PatchSessionNotFound { path: PathBuf },

    #[error("Invalid patch file at {path:?}: {reason}")]
    PatchInvalid { path: PathBuf, reason: String },
}
