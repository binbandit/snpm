use super::download::parse_sha256_checksum;
use super::platform::{binary_path_for_version, build_download_urls, platform_info};

#[test]
fn parse_sha256_checksum_valid() {
    let hex = "a".repeat(64);
    let text = format!("{}  filename.tar.gz", hex);
    let result = parse_sha256_checksum(&text).unwrap();
    assert_eq!(result, hex.as_str());
}

#[test]
fn parse_sha256_checksum_standalone() {
    let hex = "abcdef0123456789".repeat(4);
    assert_eq!(hex.len(), 64);
    let result = parse_sha256_checksum(&hex).unwrap();
    assert_eq!(result, hex.as_str());
}

#[test]
fn parse_sha256_checksum_too_short() {
    let result = parse_sha256_checksum("abcdef");
    assert!(result.is_err());
}

#[test]
fn parse_sha256_checksum_non_hex() {
    let text = "g".repeat(64);
    let result = parse_sha256_checksum(&text);
    assert!(result.is_err());
}

#[test]
fn parse_sha256_checksum_empty() {
    let result = parse_sha256_checksum("");
    assert!(result.is_err());
}

#[test]
fn binary_path_for_version_unix() {
    let dir = std::path::Path::new("/tmp/versions/2026.1.0");
    let path = binary_path_for_version(dir);
    if cfg!(windows) {
        assert_eq!(path, dir.join("snpm.exe"));
    } else {
        assert_eq!(path, dir.join("snpm"));
    }
}

#[test]
fn build_download_urls_contains_version() {
    let urls = build_download_urls("2026.1.0").unwrap();
    assert!(!urls.is_empty());
    for url in &urls {
        assert!(url.contains("v2026.1.0"));
    }
}

#[test]
fn build_download_urls_platform_specific() {
    let urls = build_download_urls("2026.1.0").unwrap();
    for url in &urls {
        if cfg!(target_os = "macos") {
            assert!(url.contains("macos") || url.contains("darwin"));
        } else if cfg!(target_os = "linux") {
            assert!(url.contains("linux"));
        } else if cfg!(target_os = "windows") {
            assert!(url.contains("windows") || url.contains("win32"));
        }
    }
}

#[test]
fn platform_info_has_valid_fields() {
    let info = platform_info();
    assert!(!info.release_os.is_empty());
    assert!(!info.release_arch.is_empty());
    assert!(!info.legacy_os.is_empty());
    assert!(!info.legacy_arch.is_empty());
    assert!(!info.ext.is_empty());
}
