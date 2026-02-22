use crate::config::OfflineMode;
use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};
use flate2::read::GzDecoder;
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
            unpack_tarball(&pkg_dir, bytes)?;
        }
    } else {
        let download_started = Instant::now();
        let bytes = download_tarball(config, &package.tarball, client).await?;
        let download_elapsed = download_started.elapsed();
        console::verbose(&format!(
            "downloaded tarball for {}@{} ({} bytes) in {:.3}s",
            package.id.name,
            package.id.version,
            bytes.len(),
            download_elapsed.as_secs_f64()
        ));

        let unpack_started = Instant::now();
        unpack_tarball(&pkg_dir, bytes)?;
        let unpack_elapsed = unpack_started.elapsed();
        console::verbose(&format!(
            "unpacked tarball for {}@{} in {:.3}s",
            package.id.name,
            package.id.version,
            unpack_elapsed.as_secs_f64()
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

fn package_root_dir(pkg_dir: &PathBuf) -> PathBuf {
    let candidate = pkg_dir.join("package");
    if candidate.is_dir() {
        candidate
    } else {
        pkg_dir.clone()
    }
}

async fn download_tarball(
    config: &SnpmConfig,
    url: &str,
    client: &reqwest::Client,
) -> Result<Vec<u8>> {
    let mut request = client.get(url);

    if let Some(token) = config.auth_token_for_url(url) {
        let header_value = format!("Bearer {}", token);
        request = request.header("authorization", header_value);
    }

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.to_string(),
        source,
    })?;

    let bytes = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?
        .bytes()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    Ok(bytes.to_vec())
}

fn unpack_tarball(pkg_dir: &PathBuf, data: Vec<u8>) -> Result<()> {
    let cursor = Cursor::new(data);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    let entries = archive.entries().map_err(|source| SnpmError::Archive {
        path: pkg_dir.clone(),
        source,
    })?;

    for entry in entries {
        let mut entry = entry.map_err(|source| SnpmError::Archive {
            path: pkg_dir.clone(),
            source,
        })?;

        let rel_path = entry.path().map_err(|source| SnpmError::Archive {
            path: pkg_dir.clone(),
            source,
        })?;

        let Some(dest_path) = safe_join(pkg_dir, &rel_path) else {
            return Err(SnpmError::Archive {
                path: pkg_dir.clone(),
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
                path: pkg_dir.clone(),
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
    use super::unpack_tarball;
    use flate2::Compression;
    use flate2::write::GzEncoder;
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
        let result = unpack_tarball(&temp.path().to_path_buf(), bytes);
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
        let result = unpack_tarball(&temp.path().to_path_buf(), bytes);
        assert!(result.is_err());
    }
}
