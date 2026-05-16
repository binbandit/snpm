use crate::{Result, SnpmConfig, SnpmError, console, http};

use super::index::distro_url;
use super::platform::{NodeArchive, NodePlatform};

use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

const COMPLETE_MARKER: &str = ".snpm_complete";

pub struct InstallSummary {
    pub version: String,
    pub bin_path: PathBuf,
    pub already_installed: bool,
}

pub async fn install_version(config: &SnpmConfig, version_with_v: &str) -> Result<InstallSummary> {
    let platform = NodePlatform::detect()?;
    let version_dir = config.node_version_dir(version_with_v);
    let bin_path = node_binary_path(&version_dir);

    if version_dir.join(COMPLETE_MARKER).is_file() && bin_path.is_file() {
        return Ok(InstallSummary {
            version: version_with_v.to_string(),
            bin_path,
            already_installed: true,
        });
    }

    fs::create_dir_all(config.node_versions_dir()).map_err(|source| SnpmError::WriteFile {
        path: config.node_versions_dir(),
        source,
    })?;

    let temp_dir = version_dir.with_extension("tmp");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|source| SnpmError::WriteFile {
            path: temp_dir.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&temp_dir).map_err(|source| SnpmError::WriteFile {
        path: temp_dir.clone(),
        source,
    })?;

    let asset = platform.asset_filename(version_with_v);
    let asset_url = format!("{}/{}/{}", distro_url(), version_with_v, asset);
    let shasums_url = format!("{}/{}/SHASUMS256.txt", distro_url(), version_with_v);

    console::step(&format!(
        "Downloading Node {} ({}-{})",
        version_with_v, platform.os, platform.arch
    ));

    let bytes = download(&asset_url).await?;

    if !skip_checksum() {
        let expected = fetch_expected_checksum(&shasums_url, &asset).await?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != expected.to_ascii_lowercase() {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(SnpmError::Tarball {
                url: asset_url,
                reason: format!("checksum mismatch (expected {expected}, got {actual})"),
            });
        }
    }

    match platform.archive {
        NodeArchive::TarGz => extract_tarball(&bytes, &temp_dir)?,
        NodeArchive::Zip => extract_zip(&bytes, &temp_dir)?,
    }

    let temp_bin = node_binary_path(&temp_dir);
    if !temp_bin.is_file() {
        let _ = fs::remove_dir_all(&temp_dir);
        return Err(SnpmError::Tarball {
            url: asset_url,
            reason: "Node binary missing from extracted archive".into(),
        });
    }

    fs::write(temp_dir.join(COMPLETE_MARKER), []).map_err(|source| SnpmError::WriteFile {
        path: temp_dir.join(COMPLETE_MARKER),
        source,
    })?;

    if version_dir.exists() {
        fs::remove_dir_all(&version_dir).map_err(|source| SnpmError::WriteFile {
            path: version_dir.clone(),
            source,
        })?;
    }

    fs::rename(&temp_dir, &version_dir).map_err(|source| SnpmError::WriteFile {
        path: version_dir.clone(),
        source,
    })?;

    Ok(InstallSummary {
        version: version_with_v.to_string(),
        bin_path: node_binary_path(&version_dir),
        already_installed: false,
    })
}

pub fn node_binary_path(version_dir: &Path) -> PathBuf {
    if cfg!(windows) {
        version_dir.join("node.exe")
    } else {
        version_dir.join("bin").join("node")
    }
}

async fn download(url: &str) -> Result<Vec<u8>> {
    let client = http::create_client()?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    if !response.status().is_success() {
        return Err(SnpmError::Tarball {
            url: url.to_string(),
            reason: format!("HTTP {}", response.status()),
        });
    }

    let mut buffer = Vec::with_capacity(response.content_length().unwrap_or(0) as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer)
}

async fn fetch_expected_checksum(shasums_url: &str, asset: &str) -> Result<String> {
    let client = http::create_client()?;
    let response = client
        .get(shasums_url)
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: shasums_url.to_string(),
            source,
        })?;

    if !response.status().is_success() {
        return Err(SnpmError::Tarball {
            url: shasums_url.to_string(),
            reason: format!("HTTP {}", response.status()),
        });
    }

    let text = response.text().await.map_err(|source| SnpmError::Http {
        url: shasums_url.to_string(),
        source,
    })?;

    parse_checksum(&text, asset).ok_or_else(|| SnpmError::Tarball {
        url: shasums_url.to_string(),
        reason: format!("checksum entry not found for {asset}"),
    })
}

fn parse_checksum(text: &str, asset: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let checksum = parts.next()?;
        let filename = parts.next()?;
        if filename == asset && checksum.len() == 64 {
            return Some(checksum.to_string());
        }
    }
    None
}

fn skip_checksum() -> bool {
    matches!(
        std::env::var("SNPM_NODE_SKIP_CHECKSUM").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

fn extract_tarball(bytes: &[u8], destination: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let mut archive = Archive::new(GzDecoder::new(Cursor::new(bytes)));
    archive.set_preserve_permissions(true);
    archive.set_overwrite(true);

    for entry in archive.entries().map_err(|source| SnpmError::Archive {
        path: destination.to_path_buf(),
        source,
    })? {
        let mut entry = entry.map_err(|source| SnpmError::Archive {
            path: destination.to_path_buf(),
            source,
        })?;

        let entry_path = entry.path().map_err(|source| SnpmError::Archive {
            path: destination.to_path_buf(),
            source,
        })?;

        let Some(rel) = strip_first_component(&entry_path) else {
            continue;
        };

        let Some(target) = safe_join(destination, &rel) else {
            return Err(SnpmError::Archive {
                path: destination.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("entry escapes destination: {}", entry_path.display()),
                ),
            });
        };

        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(&target).map_err(|source| SnpmError::WriteFile {
                path: target,
                source,
            })?;
            continue;
        }

        ensure_parent(&target)?;

        if entry_type.is_symlink() {
            let link_name = entry
                .link_name()
                .map_err(|source| SnpmError::Archive {
                    path: target.clone(),
                    source,
                })?
                .ok_or_else(|| SnpmError::Archive {
                    path: target.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "symlink missing target",
                    ),
                })?;
            create_symlink(&link_name, &target)?;
            continue;
        }

        if entry_type.is_file() {
            entry.unpack(&target).map_err(|source| SnpmError::Archive {
                path: target,
                source,
            })?;
        }
    }

    Ok(())
}

fn extract_zip(bytes: &[u8], destination: &Path) -> Result<()> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| SnpmError::Archive {
            path: destination.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()),
        })?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| SnpmError::Archive {
                path: destination.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()),
            })?;

        let Some(enclosed) = file.enclosed_name() else {
            continue;
        };

        let Some(rel) = strip_first_component(&enclosed) else {
            continue;
        };

        let Some(target) = safe_join(destination, &rel) else {
            return Err(SnpmError::Archive {
                path: destination.to_path_buf(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "zip entry escapes destination".to_string(),
                ),
            });
        };

        if file.is_dir() {
            fs::create_dir_all(&target).map_err(|source| SnpmError::WriteFile {
                path: target,
                source,
            })?;
            continue;
        }

        ensure_parent(&target)?;

        let mut buffer = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buffer)
            .map_err(|source| SnpmError::Archive {
                path: target.clone(),
                source,
            })?;
        fs::write(&target, &buffer).map_err(|source| SnpmError::WriteFile {
            path: target.clone(),
            source,
        })?;
    }

    Ok(())
}

fn strip_first_component(path: &Path) -> Option<PathBuf> {
    let mut components = path.components();
    components.next()?;
    let rest: PathBuf = components.as_path().to_path_buf();
    if rest.as_os_str().is_empty() {
        None
    } else {
        Some(rest)
    }
}

fn safe_join(base: &Path, rel: &Path) -> Option<PathBuf> {
    let mut result = base.to_path_buf();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(part) => result.push(part),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(result)
}

fn ensure_parent(target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(link_target: &Path, link_path: &Path) -> Result<()> {
    if link_path.exists() || link_path.is_symlink() {
        fs::remove_file(link_path).map_err(|source| SnpmError::WriteFile {
            path: link_path.to_path_buf(),
            source,
        })?;
    }
    std::os::unix::fs::symlink(link_target, link_path).map_err(|source| SnpmError::WriteFile {
        path: link_path.to_path_buf(),
        source,
    })
}

#[cfg(windows)]
fn create_symlink(link_target: &Path, link_path: &Path) -> Result<()> {
    if link_path.exists() {
        let _ = fs::remove_file(link_path);
    }
    let target_path = link_path
        .parent()
        .map(|parent| parent.join(link_target))
        .unwrap_or_else(|| link_target.to_path_buf());

    if target_path.is_dir() {
        std::os::windows::fs::symlink_dir(link_target, link_path).map_err(|source| {
            SnpmError::WriteFile {
                path: link_path.to_path_buf(),
                source,
            }
        })
    } else {
        match std::os::windows::fs::symlink_file(link_target, link_path) {
            Ok(()) => Ok(()),
            Err(_) => fs::copy(&target_path, link_path)
                .map(|_| ())
                .map_err(|source| SnpmError::WriteFile {
                    path: link_path.to_path_buf(),
                    source,
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_checksum, safe_join, strip_first_component};
    use std::path::{Path, PathBuf};

    #[test]
    fn extracts_first_segment() {
        let path = Path::new("node-v20.10.0-darwin-arm64/bin/node");
        let rest = strip_first_component(path).unwrap();
        assert_eq!(rest, PathBuf::from("bin/node"));
    }

    #[test]
    fn strip_returns_none_for_top_level() {
        assert!(strip_first_component(Path::new("node-v20.10.0-darwin-arm64")).is_none());
    }

    #[test]
    fn safe_join_blocks_traversal() {
        let base = Path::new("/tmp/base");
        assert!(safe_join(base, Path::new("../etc/passwd")).is_none());
        assert_eq!(
            safe_join(base, Path::new("bin/node")).unwrap(),
            PathBuf::from("/tmp/base/bin/node")
        );
    }

    #[test]
    fn parses_shasum_entry() {
        let text = "deadbeef  node-v20.10.0-darwin-arm64.tar.gz\nbadc0ffee  other.tar.gz\n";
        assert!(parse_checksum(text, "node-v20.10.0-darwin-arm64.tar.gz").is_none());

        let real = format!("{}  node-v20.10.0-darwin-arm64.tar.gz", "a".repeat(64));
        assert_eq!(
            parse_checksum(&real, "node-v20.10.0-darwin-arm64.tar.gz").as_deref(),
            Some("a".repeat(64).as_str())
        );
    }
}
