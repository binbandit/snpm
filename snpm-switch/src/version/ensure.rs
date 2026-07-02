use crate::config;

use std::fs;
use std::path::PathBuf;

use super::download::{download_version, fetch_latest_version};
use super::platform::binary_path_for_version;

pub fn ensure_version(version: &str) -> anyhow::Result<PathBuf> {
    validate_version(version)?;

    let versions_dir = config::versions_dir()?;
    let version_dir = versions_dir.join(version);
    let complete_marker = version_dir.join(".snpm_complete");
    let binary_path = binary_path_for_version(&version_dir);

    if complete_marker.is_file() && binary_path.is_file() {
        return Ok(binary_path);
    }

    // Append rather than `with_extension`: that would replace the text
    // after the last dot, so v1.2.3 and v1.2.4 would collide on the
    // same temp dir.
    let temp_dir = versions_dir.join(format!("{version}.tmp"));
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

/// The version string comes verbatim from an untrusted `package.json`
/// `packageManager` field and is used both as a path segment and in the
/// download URL. Restrict it to semver-ish characters so values like
/// `..` or `../../x` can't escape the versions directory.
fn validate_version(version: &str) -> anyhow::Result<()> {
    let valid = !version.is_empty()
        && !version.starts_with('.')
        && !version.contains("..")
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));

    if !valid {
        anyhow::bail!("Invalid snpm version {version:?} in packageManager field");
    }
    Ok(())
}

pub fn ensure_latest() -> anyhow::Result<PathBuf> {
    let latest_version = fetch_latest_version()?;
    ensure_version(&latest_version)
}

#[cfg(test)]
mod tests {
    use super::validate_version;

    #[test]
    fn accepts_normal_versions() {
        assert!(validate_version("1.2.3").is_ok());
        assert!(validate_version("2026.5.26").is_ok());
        assert!(validate_version("1.0.0-beta.1").is_ok());
        assert!(validate_version("1.0.0+build.5").is_ok());
    }

    #[test]
    fn rejects_path_traversal_and_separators() {
        assert!(validate_version("..").is_err());
        assert!(validate_version("../../etc").is_err());
        assert!(validate_version("1.2.3/../evil").is_err());
        assert!(validate_version("a/b").is_err());
        assert!(validate_version("a\\b").is_err());
        assert!(validate_version("").is_err());
        assert!(validate_version(".hidden").is_err());
    }
}
