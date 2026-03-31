use crate::config;
use std::path::{Path, PathBuf};

pub(super) fn binary_path_for_version(version_dir: &Path) -> PathBuf {
    let binary_name = if cfg!(windows) { "snpm.exe" } else { "snpm" };
    version_dir.join(binary_name)
}

pub(super) fn build_download_urls(version: &str) -> anyhow::Result<Vec<String>> {
    let base_url = config::download_base_url();
    let platform = platform_info();

    let mut asset_names = vec![format!(
        "snpm-{}-{}.{}",
        platform.release_os, platform.release_arch, platform.ext
    )];

    let legacy_asset_name = format!(
        "snpm-{}-{}.{}",
        platform.legacy_os, platform.legacy_arch, platform.ext
    );

    if !asset_names.contains(&legacy_asset_name) {
        asset_names.push(legacy_asset_name);
    }

    Ok(asset_names
        .into_iter()
        .map(|asset_name| format!("{}/v{}/{}", base_url, version, asset_name))
        .collect())
}

pub(super) struct PlatformInfo {
    pub(super) release_os: &'static str,
    pub(super) release_arch: &'static str,
    pub(super) legacy_os: &'static str,
    pub(super) legacy_arch: &'static str,
    pub(super) ext: &'static str,
}

pub(super) fn platform_info() -> PlatformInfo {
    let (release_os, legacy_os) = if cfg!(target_os = "macos") {
        ("macos", "darwin")
    } else if cfg!(target_os = "windows") {
        ("windows", "win32")
    } else {
        ("linux", "linux")
    };

    let (release_arch, legacy_arch) = if cfg!(target_arch = "aarch64") {
        ("arm64", "arm64")
    } else {
        ("amd64", "x64")
    };

    let ext = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };

    PlatformInfo {
        release_os,
        release_arch,
        legacy_os,
        legacy_arch,
        ext,
    }
}
