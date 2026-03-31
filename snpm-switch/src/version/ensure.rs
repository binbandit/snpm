use crate::config;

use std::fs;
use std::path::PathBuf;

use super::download::{download_version, fetch_latest_version};
use super::platform::binary_path_for_version;

pub fn ensure_version(version: &str) -> anyhow::Result<PathBuf> {
    let version_dir = config::versions_dir()?.join(version);
    let complete_marker = version_dir.join(".snpm_complete");
    let binary_path = binary_path_for_version(&version_dir);

    if complete_marker.is_file() && binary_path.is_file() {
        return Ok(binary_path);
    }

    let temp_dir = version_dir.with_extension("tmp");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }

    download_version(version, &temp_dir)?;

    let temp_binary = binary_path_for_version(&temp_dir);
    if !temp_binary.is_file() {
        anyhow::bail!(
            "Downloaded snpm {} but binary not found at expected path",
            version
        );
    }

    fs::write(temp_dir.join(".snpm_complete"), [])?;

    if version_dir.exists() {
        fs::remove_dir_all(&version_dir)?;
    }
    fs::rename(&temp_dir, &version_dir)?;

    Ok(binary_path)
}

pub fn ensure_latest() -> anyhow::Result<PathBuf> {
    let latest_version = fetch_latest_version()?;
    ensure_version(&latest_version)
}
