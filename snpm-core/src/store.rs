use crate::config::OfflineMode;
use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};
use base64::Engine;
use flate2::read::GzDecoder;
use futures::StreamExt;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::Cursor;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;
use tar::Archive;

/// Ensure a package is in the store (Online mode).
pub async fn ensure_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
) -> Result<PathBuf> {
    ensure_package_with_offline(config, package, client, OfflineMode::Online).await
}

/// Ensure a package is in the store, respecting offline mode.
pub async fn ensure_package_with_offline(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
    offline_mode: OfflineMode,
) -> Result<PathBuf> {
    let start = Instant::now();

    let base = config.packages_dir();
    let name_dir = sanitize_name(&package.id.name);
    let pkg_dir = base.join(name_dir).join(&package.id.version);

    let marker = pkg_dir.join(".snpm_complete");
    if marker.is_file() {
        let root = package_root_dir(&pkg_dir);
        console::verbose(&format!(
            "store hit: {}@{} ({})",
            package.id.name,
            package.id.version,
            root.display()
        ));
        return Ok(root);
    }

    // In Offline mode, we can't download - fail if not in store
    if matches!(offline_mode, OfflineMode::Offline) {
        return Err(SnpmError::OfflineRequired {
            resource: format!("package {}@{}", package.id.name, package.id.version),
        });
    }

    console::verbose(&format!(
        "store miss: {}@{}; downloading from {}",
        package.id.name, package.id.version, package.tarball
    ));

    if pkg_dir.exists() {
        fs::remove_dir_all(&pkg_dir).map_err(|source| SnpmError::WriteFile {
            path: pkg_dir.clone(),
            source,
        })?;
    }

    fs::create_dir_all(&pkg_dir).map_err(|source| SnpmError::WriteFile {
        path: pkg_dir.clone(),
        source,
    })?;

    if package.tarball.starts_with("file://") {
        let path_str = package.tarball.strip_prefix("file://").unwrap();
        let source_path = PathBuf::from(path_str);

        console::verbose(&format!(
            "installing local package from {}",
            source_path.display()
        ));

        if source_path.is_dir() {
            let dest_dir = pkg_dir.join("package");
            copy_dir_all(&source_path, &dest_dir).map_err(|e| SnpmError::Io {
                path: dest_dir.clone(),
                source: e,
            })?;
        } else {
            let bytes = fs::read(&source_path).map_err(|source| SnpmError::ReadFile {
                path: source_path.clone(),
                source,
            })?;
            verify_integrity(&package.tarball, package.integrity.as_deref(), &bytes)?;
            unpack_tarball(&pkg_dir, bytes)?;
        }
    } else {
        let download_started = Instant::now();
        let bytes = download_and_verify_tarball(
            config,
            &package.tarball,
            package.integrity.as_deref(),
            client,
        )
        .await?;
        console::verbose(&format!(
            "downloaded and verified tarball for {}@{} ({} bytes) in {:.3}s",
            package.id.name,
            package.id.version,
            bytes.len(),
            download_started.elapsed().as_secs_f64()
        ));

        let unpack_started = Instant::now();
        unpack_tarball(&pkg_dir, bytes)?;
        console::verbose(&format!(
            "unpacked tarball for {}@{} in {:.3}s",
            package.id.name,
            package.id.version,
            unpack_started.elapsed().as_secs_f64()
        ));
    }

    fs::write(&marker, []).map_err(|source| SnpmError::WriteFile {
        path: marker.clone(),
        source,
    })?;

    let root = package_root_dir(&pkg_dir);
    console::verbose(&format!(
        "ensure_package complete for {}@{} in {:.3}s (root={})",
        package.id.name,
        package.id.version,
        start.elapsed().as_secs_f64(),
        root.display()
    ));

    Ok(root)
}

fn sanitize_name(name: &str) -> String {
    name.replace('/', "_")
}

fn package_root_dir(pkg_dir: &Path) -> PathBuf {
    // npm tarballs typically extract into a `package/` directory
    let candidate = pkg_dir.join("package");
    if candidate.is_dir() {
        return candidate;
    }

    // Some packages (e.g. @types/node v25) use a different top-level directory
    // name. Find the single subdirectory that contains a package.json.
    if let Ok(entries) = fs::read_dir(pkg_dir) {
        let mut dirs: Vec<PathBuf> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.file_name().is_some_and(|n| n != ".snpm_complete") {
                dirs.push(path);
            }
        }
        if dirs.len() == 1 && dirs[0].join("package.json").is_file() {
            return dirs[0].clone();
        }
    }

    pkg_dir.to_path_buf()
}

/// Stream download while computing integrity hash in a single pass.
/// Verifies the hash after download completes, before returning bytes for extraction.
async fn download_and_verify_tarball(
    config: &SnpmConfig,
    url: &str,
    integrity: Option<&str>,
    client: &reqwest::Client,
) -> Result<Vec<u8>> {
    let mut request = client.get(url);

    if let Some(header_value) = config.authorization_header_for_url(url) {
        request = request.header("authorization", header_value);
    }

    let response = request
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    let content_length = response.content_length().unwrap_or(0) as usize;
    let mut buffer = Vec::with_capacity(content_length);

    let algorithm = integrity
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            value
                .split_whitespace()
                .next()
                .and_then(|token| token.split_once('-'))
                .map(|(algorithm, _)| algorithm.to_string())
        });

    let mut sha512_hasher = if algorithm.as_deref() == Some("sha512") {
        Some(Sha512::new())
    } else {
        None
    };
    let mut sha256_hasher = if algorithm.as_deref() == Some("sha256") {
        Some(Sha256::new())
    } else {
        None
    };
    let mut sha1_hasher = if algorithm.as_deref() == Some("sha1") {
        Some(Sha1::new())
    } else {
        None
    };

    let mut body_stream = response.bytes_stream();

    while let Some(chunk_result) = body_stream.next().await {
        let chunk = chunk_result.map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

        if let Some(ref mut hasher) = sha512_hasher {
            hasher.update(&chunk);
        }
        if let Some(ref mut hasher) = sha256_hasher {
            hasher.update(&chunk);
        }
        if let Some(ref mut hasher) = sha1_hasher {
            hasher.update(&chunk);
        }

        buffer.extend_from_slice(&chunk);
    }

    if let Some(integrity_str) = integrity.map(str::trim).filter(|value| !value.is_empty()) {
        let computed_digest = sha512_hasher
            .map(|hasher| ("sha512", hasher.finalize().to_vec()))
            .or_else(|| sha256_hasher.map(|hasher| ("sha256", hasher.finalize().to_vec())))
            .or_else(|| sha1_hasher.map(|hasher| ("sha1", hasher.finalize().to_vec())));

        if let Some((algorithm_name, actual_digest)) = computed_digest {
            let matched = integrity_str.split_whitespace().any(|token| {
                let token = token.split('?').next().unwrap_or(token);
                if let Some((alg, expected_b64)) = token.split_once('-')
                    && alg == algorithm_name
                    && let Ok(expected) =
                        base64::engine::general_purpose::STANDARD.decode(expected_b64)
                {
                    return actual_digest == expected;
                }
                false
            });

            if !matched {
                return Err(SnpmError::Tarball {
                    url: url.to_string(),
                    reason: "integrity verification failed".to_string(),
                });
            }
        }
    }

    Ok(buffer)
}

fn verify_integrity(url: &str, integrity: Option<&str>, bytes: &[u8]) -> Result<()> {
    let Some(integrity) = integrity.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut matched_supported = false;
    let mut saw_supported = false;

    for token in integrity.split_whitespace() {
        let token = token.split('?').next().unwrap_or(token);
        let Some((algorithm, expected_digest)) = token.split_once('-') else {
            continue;
        };

        let actual = match algorithm {
            "sha512" => {
                saw_supported = true;
                Sha512::digest(bytes).to_vec()
            }
            "sha256" => {
                saw_supported = true;
                Sha256::digest(bytes).to_vec()
            }
            "sha1" => {
                saw_supported = true;
                Sha1::digest(bytes).to_vec()
            }
            _ => continue,
        };

        let expected = base64::engine::general_purpose::STANDARD
            .decode(expected_digest)
            .map_err(|error| SnpmError::Tarball {
                url: url.to_string(),
                reason: format!("invalid integrity value: {error}"),
            })?;

        if actual == expected {
            matched_supported = true;
            break;
        }
    }

    if !saw_supported {
        console::warn(&format!(
            "Skipping integrity verification for {}: unsupported algorithm",
            url
        ));
        return Ok(());
    }

    if matched_supported {
        Ok(())
    } else {
        Err(SnpmError::Tarball {
            url: url.to_string(),
            reason: "integrity verification failed".to_string(),
        })
    }
}

fn unpack_tarball(pkg_dir: &Path, data: Vec<u8>) -> Result<()> {
    let cursor = Cursor::new(data);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    let entries = archive.entries().map_err(|source| SnpmError::Archive {
        path: pkg_dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let mut entry = entry.map_err(|source| SnpmError::Archive {
            path: pkg_dir.to_path_buf(),
            source,
        })?;

        let rel_path = entry.path().map_err(|source| SnpmError::Archive {
            path: pkg_dir.to_path_buf(),
            source,
        })?;

        let Some(dest_path) = safe_join(pkg_dir, &rel_path) else {
            return Err(SnpmError::Archive {
                path: pkg_dir.to_path_buf(),
                source: std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "archive entry escapes extraction root: {}",
                        rel_path.display()
                    ),
                ),
            });
        };

        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(SnpmError::Archive {
                path: pkg_dir.to_path_buf(),
                source: std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "archive contains forbidden symlink/hardlink entry: {}",
                        rel_path.display()
                    ),
                ),
            });
        }

        if entry_type.is_dir() {
            fs::create_dir_all(&dest_path).map_err(|source| SnpmError::WriteFile {
                path: dest_path.clone(),
                source,
            })?;
            continue;
        }

        if !entry_type.is_file() {
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        entry
            .unpack(&dest_path)
            .map_err(|source| SnpmError::Archive {
                path: dest_path,
                source,
            })?;
    }

    Ok(())
}

fn safe_join(root: &Path, rel: &Path) -> Option<PathBuf> {
    let mut out = root.to_path_buf();
    for component in rel.components() {
        match component {
            Component::Normal(segment) => out.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let name = entry.file_name();

        if name == ".git" || name == "node_modules" {
            continue;
        }

        if ty.is_symlink() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "refusing to copy symlink from local dependency: {}",
                    entry.path().display()
                ),
            ));
        }

        let dst_path = dst.join(&name);
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{unpack_tarball, verify_integrity};
    use base64::Engine;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest, Sha512};
    use std::io::Write;
    use tar::{Builder, EntryType, Header};
    use tempfile::tempdir;

    fn build_tarball<F>(mut append: F) -> Vec<u8>
    where
        F: FnMut(&mut Builder<Vec<u8>>),
    {
        let mut builder = Builder::new(Vec::new());
        append(&mut builder);
        let tar_bytes = builder.into_inner().unwrap();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn rejects_symlink_entry() {
        let bytes = build_tarball(|builder| {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_path("package/symlink").unwrap();
            header.set_link_name("../../outside").unwrap();
            header.set_size(0);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, std::io::empty()).unwrap();
        });

        let temp = tempdir().unwrap();
        let result = unpack_tarball(temp.path(), bytes);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_hardlink_entry() {
        let bytes = build_tarball(|builder| {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Link);
            header.set_path("package/link").unwrap();
            header.set_link_name("../../outside").unwrap();
            header.set_size(0);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, std::io::empty()).unwrap();
        });

        let temp = tempdir().unwrap();
        let result = unpack_tarball(temp.path(), bytes);
        assert!(result.is_err());
    }

    #[test]
    fn package_root_dir_finds_standard_package_dir() {
        let temp = tempdir().unwrap();
        let pkg_dir = temp.path();
        std::fs::create_dir_all(pkg_dir.join("package")).unwrap();
        std::fs::write(pkg_dir.join("package/package.json"), "{}").unwrap();

        let root = super::package_root_dir(pkg_dir);
        assert_eq!(root, pkg_dir.join("package"));
    }

    #[test]
    fn package_root_dir_finds_nonstandard_toplevel_dir() {
        // Simulates @types/node v25 where tarball uses "node/" instead of "package/"
        let temp = tempdir().unwrap();
        let pkg_dir = temp.path();
        std::fs::create_dir_all(pkg_dir.join("node")).unwrap();
        std::fs::write(pkg_dir.join("node/package.json"), "{}").unwrap();
        // .snpm_complete marker should not confuse the detection
        std::fs::write(pkg_dir.join(".snpm_complete"), "").unwrap();

        let root = super::package_root_dir(pkg_dir);
        assert_eq!(root, pkg_dir.join("node"));
    }

    #[test]
    fn package_root_dir_returns_pkg_dir_when_flat() {
        let temp = tempdir().unwrap();
        let pkg_dir = temp.path();
        std::fs::write(pkg_dir.join("package.json"), "{}").unwrap();
        std::fs::write(pkg_dir.join("index.js"), "").unwrap();

        let root = super::package_root_dir(pkg_dir);
        assert_eq!(root, pkg_dir.to_path_buf());
    }

    #[test]
    fn verifies_sha512_integrity() {
        let bytes = b"hello world";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let integrity = format!("sha512-{}", digest);

        assert!(
            verify_integrity(
                "https://registry.example.com/pkg.tgz",
                Some(&integrity),
                bytes
            )
            .is_ok()
        );
        assert!(
            verify_integrity(
                "https://registry.example.com/pkg.tgz",
                Some(&integrity),
                b"tampered"
            )
            .is_err()
        );
    }
}
