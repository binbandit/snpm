use crate::console;
use crate::resolve::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};
use flate2::read::GzDecoder;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::Instant;
use tar::Archive;

pub async fn ensure_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
) -> Result<PathBuf> {
    let start = Instant::now();

    let base = config.packages_dir();
    let name_dir = sanitize_name(&package.id.name);
    let pkg_dir = base.join(name_dir).join(&package.id.version);

    let marker = pkg_dir.join(".snpm_complete");
    if marker.is_file() {
        let root = package_root_dir(&pkg_dir);
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "store hit: {}@{} ({})",
                package.id.name,
                package.id.version,
                root.display()
            ));
        }
        return Ok(root);
    }

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "store miss: {}@{}; downloading from {}",
            package.id.name, package.id.version, package.tarball
        ));
    }

    fs::create_dir_all(&pkg_dir).map_err(|source| SnpmError::WriteFile {
        path: pkg_dir.clone(),
        source,
    })?;

    if package.tarball.starts_with("file://") {
        let path_str = package.tarball.strip_prefix("file://").unwrap();
        let source_path = PathBuf::from(path_str);

        if console::is_logging_enabled() {
            console::verbose(&format!(
                "installing local package from {}",
                source_path.display()
            ));
        }

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
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "downloaded tarball for {}@{} ({} bytes) in {:.3}s",
                package.id.name,
                package.id.version,
                bytes.len(),
                download_elapsed.as_secs_f64()
            ));
        }

        let unpack_started = Instant::now();
        unpack_tarball(&pkg_dir, bytes)?;
        let unpack_elapsed = unpack_started.elapsed();
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "unpacked tarball for {}@{} in {:.3}s",
                package.id.name,
                package.id.version,
                unpack_elapsed.as_secs_f64()
            ));
        }
    }

    fs::write(&marker, []).map_err(|source| SnpmError::WriteFile {
        path: marker.clone(),
        source,
    })?;

    let root = package_root_dir(&pkg_dir);
    if console::is_logging_enabled() {
        console::verbose(&format!(
            "ensure_package complete for {}@{} in {:.3}s (root={})",
            package.id.name,
            package.id.version,
            start.elapsed().as_secs_f64(),
            root.display()
        ));
    }

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

    archive
        .unpack(pkg_dir)
        .map_err(|source| SnpmError::Archive {
            path: pkg_dir.clone(),
            source,
        })?;

    Ok(())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let name = entry.file_name();

        if name == ".git" || name == "node_modules" {
            continue;
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
