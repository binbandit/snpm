use super::extract::{extract_tarball, extract_zip};
use super::platform::{binary_path_for_version, build_download_urls};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub(super) fn download_version(version: &str, destination: &Path) -> anyhow::Result<()> {
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
            .map(|status| status.to_string())
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

pub(super) fn parse_sha256_checksum(text: &str) -> anyhow::Result<&str> {
    let checksum = text.split_whitespace().next().unwrap_or("");

    if checksum.len() != 64
        || !checksum
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        anyhow::bail!("Invalid SHA256 checksum format");
    }

    Ok(checksum)
}

pub(super) fn fetch_latest_version() -> anyhow::Result<String> {
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
    Ok(release.tag_name.trim_start_matches('v').to_string())
}
