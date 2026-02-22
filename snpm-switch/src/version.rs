use crate::config;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tar::Archive;

pub fn ensure_version(version: &str) -> anyhow::Result<PathBuf> {
    let version_dir = config::versions_dir().join(version);
    let binary_path = binary_path_for_version(&version_dir);

    if binary_path.is_file() {
        return Ok(binary_path);
    }

    download_version(version, &version_dir)?;

    if !binary_path.is_file() {
        anyhow::bail!(
            "Downloaded snpm {} but binary not found at expected path",
            version
        );
    }

    Ok(binary_path)
}

pub fn ensure_latest() -> anyhow::Result<PathBuf> {
    let latest_version = fetch_latest_version()?;
    ensure_version(&latest_version)
}

pub fn list_cached_versions() -> anyhow::Result<Vec<String>> {
    let versions_dir = config::versions_dir();

    if !versions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();

    for entry in fs::read_dir(&versions_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            versions.push(name.to_string());
        }
    }

    versions.sort();
    Ok(versions)
}

pub fn clear_cache() -> anyhow::Result<()> {
    let versions_dir = config::versions_dir();

    if versions_dir.is_dir() {
        fs::remove_dir_all(&versions_dir)?;
    }

    Ok(())
}

fn binary_path_for_version(version_dir: &Path) -> PathBuf {
    let binary_name = if cfg!(windows) { "snpm.exe" } else { "snpm" };
    version_dir.join(binary_name)
}

fn download_version(version: &str, destination: &Path) -> anyhow::Result<()> {
    eprintln!("Downloading snpm {}...", version);

    let client = reqwest::blocking::Client::builder()
        .user_agent("snpm-switch")
        .build()?;

    let mut selected = None;
    let mut last_status = None;

    for url in build_download_urls(version)? {
        let response = client.get(&url).send()?;

        if response.status().is_success() {
            selected = Some((url, response.bytes()?));
            break;
        }

        last_status = Some(response.status());
    }

    let (url, bytes) = selected.ok_or_else(|| {
        let status = last_status
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        anyhow::anyhow!(
            "Failed to download snpm {} from known release asset names (last HTTP status: {})",
            version,
            status
        )
    })?;

    verify_checksum(&client, &url, &bytes)?;

    fs::create_dir_all(destination)?;

    if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
        extract_tarball(&bytes, destination)?;
    } else if url.ends_with(".zip") {
        extract_zip(&bytes, destination)?;
    } else {
        let binary_path = binary_path_for_version(destination);
        fs::write(&binary_path, &bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&binary_path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&binary_path, permissions)?;
        }
    }

    eprintln!("Installed snpm {} to {}", version, destination.display());

    Ok(())
}

fn build_download_urls(version: &str) -> anyhow::Result<Vec<String>> {
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

fn verify_checksum(
    client: &reqwest::blocking::Client,
    url: &str,
    bytes: &[u8],
) -> anyhow::Result<()> {
    if should_skip_checksum() {
        eprintln!("Skipping checksum verification due to SNPM_SWITCH_SKIP_CHECKSUM.");
        return Ok(());
    }

    let checksum_url = format!("{}.sha256", url);
    let response = client.get(&checksum_url).send()?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to download checksum file for {}: HTTP {}",
            url,
            response.status()
        );
    }

    let checksum_text = response.text()?;
    let expected = parse_sha256_checksum(&checksum_text)?;

    let actual = format!("{:x}", Sha256::digest(bytes));

    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        anyhow::bail!(
            "Checksum mismatch for {} (expected {}, got {})",
            url,
            expected,
            actual
        )
    }
}

fn should_skip_checksum() -> bool {
    match std::env::var("SNPM_SWITCH_SKIP_CHECKSUM") {
        Ok(value) => value == "1" || value.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

fn parse_sha256_checksum(text: &str) -> anyhow::Result<&str> {
    let checksum = text.split_whitespace().next().unwrap_or("");

    if checksum.len() != 64 || !checksum.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!("Invalid SHA256 checksum format");
    }

    Ok(checksum)
}

struct PlatformInfo {
    release_os: &'static str,
    release_arch: &'static str,
    legacy_os: &'static str,
    legacy_arch: &'static str,
    ext: &'static str,
}

fn platform_info() -> PlatformInfo {
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

fn extract_tarball(data: &[u8], destination: &Path) -> anyhow::Result<()> {
    let cursor = Cursor::new(data);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if file_name == "snpm" || file_name == "snpm.exe" {
            let dest_path = destination.join(file_name);
            entry.unpack(&dest_path)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = fs::metadata(&dest_path)?.permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&dest_path, permissions)?;
            }

            return Ok(());
        }
    }

    anyhow::bail!("snpm binary not found in archive");
}

fn extract_zip(data: &[u8], destination: &Path) -> anyhow::Result<()> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_name = file.name();

        if file_name == "snpm" || file_name == "snpm.exe" {
            let dest_path = destination.join(file_name);
            let mut dest_file = fs::File::create(&dest_path)?;
            std::io::copy(&mut file, &mut dest_file)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = fs::metadata(&dest_path)?.permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&dest_path, permissions)?;
            }

            return Ok(());
        }
    }

    anyhow::bail!("snpm binary not found in archive");
}

fn fetch_latest_version() -> anyhow::Result<String> {
    let url = "https://api.github.com/repos/binbandit/snpm/releases/latest";

    let client = reqwest::blocking::Client::builder()
        .user_agent("snpm-switch")
        .build()?;

    let response = client.get(url).send()?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch latest version: HTTP {}", response.status());
    }

    #[derive(serde::Deserialize)]
    struct Release {
        tag_name: String,
    }

    let release: Release = response.json()?;
    let version = release.tag_name.trim_start_matches('v').to_string();

    Ok(version)
}
